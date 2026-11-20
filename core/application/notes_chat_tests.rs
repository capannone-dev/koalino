//! Specifies chat with notes (#97): what fits a connection's context, in what order, what the
//! model is sent, and what the reply says was included.

use std::sync::Mutex;

use chrono::{TimeZone, Utc};

use super::notes_chat::{
    ChatError, ChatMaterial, ContextItem, Inclusion, ask, ask_with_prompt, fit_context, relevance,
};
use crate::{
    domain::{
        chat::{ChatMessage, ChatRole},
        errors::LlmError,
        llm::{LlmCompletion, LlmConnection, LlmImage, LlmKind, LlmRequest, LlmRole},
        shared_prompts::ChatPrompt,
    },
    ports::ai::LlmEngine,
};

/// Answers each request in turn and keeps them.
struct ScriptedEngine {
    answers: Mutex<Vec<Result<LlmCompletion, LlmError>>>,
    seen: Mutex<Vec<LlmRequest>>,
}

impl ScriptedEngine {
    fn new(mut answers: Vec<Result<LlmCompletion, LlmError>>) -> Self {
        answers.reverse();
        Self {
            answers: Mutex::new(answers),
            seen: Mutex::new(Vec::new()),
        }
    }

    fn seen(&self) -> Vec<LlmRequest> {
        self.seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }
}

impl LlmEngine for ScriptedEngine {
    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request.clone());
        }
        self.answers
            .lock()
            .ok()
            .and_then(|mut answers| answers.pop())
            .unwrap_or(Err(LlmError::Unreachable))
    }
}

fn answer(text: &str) -> Result<LlmCompletion, LlmError> {
    Ok(LlmCompletion {
        text: text.to_owned(),
        truncated: false,
    })
}

fn connection(context_tokens: u32) -> LlmConnection {
    LlmConnection::with_id(
        "c",
        "Office",
        LlmKind::OpenAiCompatible,
        "http://127.0.0.1:8080/v1",
        "local",
        context_tokens,
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

fn item(label: &str, full_chars: usize, short_chars: Option<usize>) -> ContextItem {
    ContextItem {
        label: label.to_owned(),
        full: "f".repeat(full_chars),
        short: short_chars.map(|chars| "s".repeat(chars)),
        target: None,
    }
}

#[test]
fn everything_is_included_when_it_fits() {
    let items = [item("A", 100, Some(10)), item("B", 100, None)];
    assert_eq!(
        fit_context(&items, 10_000),
        [Inclusion::Full, Inclusion::Full]
    );
}

#[test]
fn summaries_replace_transcripts_then_the_last_items_are_dropped() {
    // Newest first: the oldest meeting is the one dropped.
    let items = [
        item("New", 1_000, Some(100)),
        item("Mid", 1_000, Some(100)),
        item("Old", 1_000, Some(100)),
    ];
    // Three short versions fit (3 x 164), plus one full upgrade (+900).
    assert_eq!(
        fit_context(&items, 1_500),
        [Inclusion::Full, Inclusion::Short, Inclusion::Short]
    );
    // Only two short versions fit.
    assert_eq!(
        fit_context(&items, 340),
        [Inclusion::Short, Inclusion::Short, Inclusion::Omitted]
    );
    // A single item larger than everything is cut, not dropped.
    assert_eq!(
        fit_context(&[item("Huge", 5_000, None)], 300),
        [Inclusion::Cut]
    );
}

#[test]
fn relevance_counts_the_questions_words_once_ignoring_case_and_accents() {
    assert_eq!(
        relevance(
            "Quando è stato approvato il budget?",
            "Il BUDGET è approvato, budget!"
        ),
        2
    );
    assert_eq!(
        relevance("who said it?", "anything"),
        0,
        "short words are ignored"
    );
}

#[test]
fn the_question_follows_the_recent_turns_and_the_material_is_in_the_instructions() {
    let engine = ScriptedEngine::new(vec![answer("On Friday, at [00:01:01].")]);
    let at = Utc
        .with_ymd_and_hms(2026, 11, 9, 12, 0, 0)
        .single()
        .unwrap_or_default();
    let history = vec![
        ChatMessage::question("Who came?", at),
        ChatMessage {
            role: ChatRole::Assistant,
            text: String::from("Anna and Bo."),
            ..ChatMessage::question("", at)
        },
    ];
    let material = ChatMaterial {
        description: String::from("the meeting \"Review\""),
        items: vec![ContextItem {
            label: String::from("Meeting \"Review\""),
            full: String::from("[00:01:01] Anna: We ship on Friday."),
            short: None,
            target: None,
        }],
        images: Vec::new(),
    };

    let reply = ask(
        &engine,
        &connection(8_000),
        material,
        &history,
        " When do we ship? ",
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(reply.text, "On Friday, at [00:01:01].");
    assert_eq!(reply.included, ["Meeting \"Review\""]);
    assert!(reply.omitted.is_empty());
    let sent = engine.seen();
    assert_eq!(sent.len(), 1);
    assert!(
        sent[0].system.contains("We ship on Friday."),
        "{}",
        sent[0].system
    );
    assert!(sent[0].system.contains("the meeting \"Review\""));
    let roles: Vec<LlmRole> = sent[0].turns.iter().map(|turn| turn.role).collect();
    assert_eq!(roles, [LlmRole::User, LlmRole::Assistant, LlmRole::User]);
    assert_eq!(sent[0].turns[2].text, "When do we ship?");
}

#[test]
fn what_did_not_fit_is_named_to_the_model_and_in_the_reply() {
    let engine = ScriptedEngine::new(vec![answer("ok")]);
    let material = ChatMaterial {
        description: String::from("the project \"Launch\""),
        items: vec![item("New", 3_000, Some(200)), item("Old", 3_000, Some(200))],
        images: Vec::new(),
    };
    // 2 048 tokens: 2 048 - 512 answer - 400 framing = about 1 130 tokens, 3 390 characters.
    let reply = ask(&engine, &connection(2_048), material, &[], "What changed?")
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(reply.included, ["New", "Old (summary only)"]);

    let engine = ScriptedEngine::new(vec![answer("ok")]);
    let material = ChatMaterial {
        description: String::from("everything"),
        items: vec![item("A", 3_000, None), item("B", 3_000, None)],
        images: Vec::new(),
    };
    let reply = ask(&engine, &connection(2_048), material, &[], "What changed?")
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(reply.included, ["A"]);
    assert_eq!(reply.omitted, ["B"]);
    assert!(engine.seen()[0].system.contains("Left out for length"));
}

#[test]
fn images_are_dropped_when_the_server_cannot_read_them() {
    let engine = ScriptedEngine::new(vec![
        Err(LlmError::Rejected {
            status: 400,
            provider_reason: None,
        }),
        answer("A whiteboard."),
    ]);
    let image = LlmImage::new("image/png", vec![1, 2, 3]).unwrap_or_else(|| panic!("an image"));
    let material = ChatMaterial {
        description: String::from("the meeting"),
        items: vec![item("Meeting", 100, None)],
        images: vec![image],
    };
    let reply = ask(
        &engine,
        &connection(32_000),
        material,
        &[],
        "What is in the photo?",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(reply.images_sent, 0);
    assert_eq!(engine.seen().len(), 2);
}

#[test]
fn bad_questions_and_empty_material_are_refused_before_sending() {
    let engine = ScriptedEngine::new(Vec::new());
    let material = || ChatMaterial {
        description: String::from("x"),
        items: vec![item("A", 10, None)],
        images: Vec::new(),
    };
    assert!(matches!(
        ask(&engine, &connection(8_000), material(), &[], "  "),
        Err(ChatError::EmptyQuestion)
    ));
    assert!(matches!(
        ask(
            &engine,
            &connection(8_000),
            material(),
            &[],
            &"x".repeat(4_001)
        ),
        Err(ChatError::QuestionTooLong)
    ));
    assert!(matches!(
        ask(
            &engine,
            &connection(8_000),
            ChatMaterial::default(),
            &[],
            "Hi?"
        ),
        Err(ChatError::NothingToChatAbout)
    ));
    assert!(engine.seen().is_empty());
}

#[test]
fn material_is_numbered_and_the_answer_cites_it_as_sources() {
    use crate::domain::chat::ChatTarget;
    let engine = ScriptedEngine::new(vec![answer(
        "We ship on Friday [2, 00:01:01]; the plan agrees [1]. Not [3].",
    )]);
    let meeting = ChatTarget::Meeting {
        meeting_id: String::from("m-1"),
    };
    let material = ChatMaterial {
        description: String::from("the project \"Launch\""),
        items: vec![
            ContextItem {
                label: String::from("File plan.md"),
                full: String::from("Ship on Friday."),
                short: None,
                target: None,
            },
            ContextItem {
                label: String::from("Meeting \"Review\""),
                full: String::from("[00:01:01] Anna: We ship on Friday."),
                short: None,
                target: Some(meeting.clone()),
            },
        ],
        images: Vec::new(),
    };

    let reply = ask(&engine, &connection(8_000), material, &[], "When?")
        .unwrap_or_else(|error| panic!("{error}"));

    let system = &engine.seen()[0].system;
    assert!(system.contains("## [1] File plan.md"), "{system}");
    assert!(system.contains("## [2] Meeting \"Review\""), "{system}");
    assert!(
        system.contains("[n, hh:mm:ss]"),
        "the model is told how to cite"
    );
    let cited: Vec<(u16, Option<u64>, Option<ChatTarget>)> = reply
        .sources
        .iter()
        .map(|source| (source.number, source.at_ms, source.target.clone()))
        .collect();
    assert_eq!(
        cited,
        vec![(2, Some(61_000), Some(meeting)), (1, None, None)]
    );
}

/// The owner's chat prompt (#145) frames the material in place of the built-in one, filled in
/// with what is asked about; the rule to cite the material by number always follows it.
#[test]
fn the_owners_chat_prompt_replaces_the_built_in_one() {
    let engine = ScriptedEngine::new(vec![answer("Friday [1].")]);
    let material = ChatMaterial {
        description: String::from("the meeting \"Review\""),
        items: vec![ContextItem {
            label: String::from("Meeting \"Review\""),
            full: String::from("[00:01:01] Anna: We ship on Friday."),
            short: None,
            target: None,
        }],
        images: Vec::new(),
    };
    let prompt = ChatPrompt::parse("Reply in one line about {{scope}}.")
        .unwrap_or_else(|error| panic!("{error}"));

    ask_with_prompt(&engine, &connection(8_000), &prompt, material, &[], "When?")
        .unwrap_or_else(|error| panic!("{error}"));

    let system = &engine.seen()[0].system;
    assert!(
        system.starts_with("Reply in one line about the meeting \"Review\"."),
        "{system}"
    );
    assert!(!system.contains("You answer the owner's questions"));
    assert!(system.contains("as [n]"), "{system}");
    assert!(system.contains("We ship on Friday."));
}
