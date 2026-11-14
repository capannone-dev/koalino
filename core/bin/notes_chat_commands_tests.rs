//! Specifies chat with notes for the hosts (#97): requests, what each scope gathers, and asking
//! through a connection with the question and answer kept in the chat.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};

use super::{
    llm_commands::{LlmServices, run_llm_command},
    notes_chat_commands::{
        ChatCommand, ask_with, gather_for_ask, gather_material, parse_chat_request,
        run_chat_store_command, store_messages,
    },
    project_commands::ProjectFiles,
};
use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher,
        meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
        meeting_edits::{edit_summary, set_notes},
    },
    domain::{
        chat::ChatScope,
        errors::LlmError,
        llm::{LlmCompletion, LlmConnection, LlmRequest},
        models::{DeviceId, MeetingId, MeetingTitle, TranscriptSegment},
        projects::ProjectName,
    },
    ports::{ai::LlmEngine, projects::ProjectRepository, storage::NoteRepository},
};

fn repository() -> SqliteNoteRepository {
    SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("{error}"))
}

fn meeting(repository: &SqliteNoteRepository, id: &str, title: &str, said: &str) -> MeetingId {
    let meeting_id = MeetingId::new(id);
    MeetingCrdtService::new(repository)
        .create_titled_meeting(
            &meeting_id,
            &MeetingTitle::parse(title).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    repository
        .save_transcript_segments(
            &meeting_id,
            &[TranscriptSegment::new(
                meeting_id.clone(),
                61_000,
                64_000,
                None,
                said,
            )],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    meeting_id
}

#[test]
fn requests_name_a_scope_and_a_command() {
    assert!(matches!(
        parse_chat_request(r#"{"command":"history","scope":"meeting","id":"m-1"}"#),
        Ok(ChatCommand::History(ChatScope::Meeting(_)))
    ));
    assert!(matches!(
        parse_chat_request(r#"{"command":"ask","scope":"all","question":"Hi?"}"#),
        Ok(ChatCommand::Ask {
            scope: ChatScope::All,
            ..
        })
    ));
    for refused in [
        "not json",
        r#"{"command":"ask","scope":"meeting"}"#,
        r#"{"command":"ask","scope":"project","id":"not-a-uuid"}"#,
        r#"{"command":"drop","scope":"all"}"#,
    ] {
        let error = parse_chat_request(refused)
            .err()
            .unwrap_or_else(|| panic!("{refused}"));
        assert_eq!(error.code, "invalid_request", "{refused}");
    }
}

#[test]
fn a_meeting_chat_gets_its_summary_notes_and_transcript() {
    let repository = repository();
    let meeting_id = meeting(
        &repository,
        "m-1",
        "Budget review",
        "The budget is approved.",
    );
    set_notes(&repository, &meeting_id, "Ask Anna about Q3.")
        .unwrap_or_else(|error| panic!("{error}"));
    edit_summary(
        &repository,
        &meeting_id,
        "Budget approved.",
        chrono::Utc::now(),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    let material = gather_material(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        None,
        &ChatScope::Meeting(meeting_id),
        "",
    )
    .unwrap_or_else(|error| panic!("{}", error.message));

    assert_eq!(material.description, "the meeting \"Budget review\"");
    assert_eq!(material.items.len(), 1);
    let full = &material.items[0].full;
    for expected in [
        "Budget approved.",
        "Ask Anna about Q3.",
        "[00:01:01] Unattributed: The budget is approved.",
    ] {
        assert!(full.contains(expected), "{full}");
    }
    let missing = gather_material(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        None,
        &ChatScope::Meeting(MeetingId::new("nope")),
        "",
    );
    assert_eq!(
        missing.err().map(|error| error.code),
        Some("meeting_not_found")
    );
}

#[test]
fn a_project_chat_gets_its_files_then_its_meetings_newest_first() {
    let folder = std::env::temp_dir().join(format!("chat-project-{}", uuid::Uuid::new_v4()));
    let repository = repository();
    let files = ProjectFiles::new(
        folder.join("attachments"),
        DeviceId::new("device"),
        repository.sealed_files(),
    );
    let project = repository
        .create_project(&ProjectName::parse("Launch").unwrap_or_else(|error| panic!("{error}")))
        .unwrap_or_else(|error| panic!("{error}"))
        .project_id();
    files
        .add_note(&repository, project, "Plan", "Ship on Friday.")
        .unwrap_or_else(|error| panic!("{error}"));
    for (id, title) in [("m-old", "Kick-off"), ("m-new", "Review")] {
        let meeting_id = meeting(&repository, id, title, "Notes.");
        repository
            .move_meeting_to_project(&meeting_id, Some(project))
            .unwrap_or_else(|error| panic!("{error}"));
        // Start times are kept to the second.
        std::thread::sleep(std::time::Duration::from_millis(1_100));
    }
    meeting(&repository, "m-out", "Elsewhere", "Not in the project.");

    let material = gather_material(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        Some(&files),
        &ChatScope::Project(project),
        "",
    )
    .unwrap_or_else(|error| panic!("{}", error.message));

    let labels: Vec<&str> = material
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(labels.len(), 3, "{labels:?}");
    assert_eq!(labels[0], "File Plan.md");
    assert!(labels[1].starts_with("Meeting \"Review\""), "{labels:?}");
    assert!(labels[2].starts_with("Meeting \"Kick-off\""), "{labels:?}");
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn everything_starts_with_a_list_then_the_best_matches() {
    let repository = repository();
    meeting(&repository, "m-1", "Standup", "Nothing special.");
    // Start times are kept to the second.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    meeting(&repository, "m-2", "Hiring", "We discussed the candidates.");
    // Start times are kept to the second.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    meeting(&repository, "m-3", "Lunch", "Pasta.");

    let material = gather_material(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        None,
        &ChatScope::All,
        "What about the candidates?",
    )
    .unwrap_or_else(|error| panic!("{}", error.message));

    assert_eq!(material.items[0].label, "List of all meetings and projects");
    assert!(material.items[0].full.contains("\"Standup\""));
    assert!(
        material.items[1].label.starts_with("Meeting \"Hiring\""),
        "the best match first"
    );
    assert!(
        material.items[2].label.starts_with("Meeting \"Lunch\""),
        "then newest first"
    );
}

/// Answers with a fixed text and keeps the requests.
struct Recorder(Arc<Mutex<Vec<LlmRequest>>>);

impl LlmEngine for Recorder {
    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        if let Ok(mut seen) = self.0.lock() {
            seen.push(request.clone());
        }
        Ok(LlmCompletion {
            text: String::from("It was approved."),
            truncated: false,
        })
    }
}

#[test]
fn asking_keeps_the_question_and_the_answer_in_the_chat() {
    let folder = std::env::temp_dir().join(format!("chat-ask-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    let seen: Arc<Mutex<Vec<LlmRequest>>> = Arc::default();
    let recorder = Arc::clone(&seen);
    let services = LlmServices::new(folder.join("db.sqlite3.llm-settings.json"), "Laptop")
        .with_transcription_settings(PathBuf::from("/nonexistent"))
        .with_engine_factory(Arc::new(move |_connection: &LlmConnection| {
            Ok(Box::new(Recorder(Arc::clone(&recorder))) as Box<dyn LlmEngine>)
        }));
    let repository = repository();
    let voices = InMemoryVoiceVectorDB::new();
    let meeting_id = meeting(
        &repository,
        "m-1",
        "Budget review",
        "The budget is approved.",
    );
    let scope = ChatScope::Meeting(meeting_id);

    let no_connection = ask_with(&services, None, "Approved?", unreachable_gather, |_| Ok(()));
    assert_eq!(no_connection["error_code"], "no_connection");

    let saved: Value = serde_json::from_str(&run_llm_command(
        &services,
        &json!({"command": "save_connection", "label": "Local", "kind": "open_ai_compatible",
                "base_url": "http://127.0.0.1:8080/v1", "model": "m", "context_tokens": 8192})
        .to_string(),
    ))
    .unwrap_or_default();
    assert_eq!(saved["ok"], true, "{saved}");

    let answer = ask_with(
        &services,
        None,
        "Was the budget approved?",
        || {
            gather_for_ask(
                &repository,
                &voices,
                None,
                &scope,
                "Was the budget approved?",
            )
        },
        |messages| store_messages(&repository, &scope, messages),
    );

    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["answer"]["text"], "It was approved.");
    assert_eq!(answer["answer"]["connection"], "Local");
    assert!(
        answer["answer"]["included"][0]
            .as_str()
            .is_some_and(|label| label.starts_with("Meeting \"Budget review\""))
    );
    let history = run_chat_store_command(&repository, &ChatCommand::History(scope.clone()));
    assert_eq!(
        history["messages"].as_array().map(Vec::len),
        Some(2),
        "{history}"
    );
    assert_eq!(history["messages"][0]["role"], "user");

    // The next question carries the chat so far.
    let _ = ask_with(
        &services,
        None,
        "By whom?",
        || gather_for_ask(&repository, &voices, None, &scope, "By whom?"),
        |messages| store_messages(&repository, &scope, messages),
    );
    let requests = seen.lock().map(|seen| seen.clone()).unwrap_or_default();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].turns.len(), 3);

    let cleared = run_chat_store_command(&repository, &ChatCommand::Clear(scope.clone()));
    assert_eq!(cleared["cleared"], true);
    let _ = std::fs::remove_dir_all(folder);
}

fn unreachable_gather() -> Result<
    (
        crate::application::notes_chat::ChatMaterial,
        Vec<crate::domain::chat::ChatMessage>,
    ),
    super::llm_commands::CommandError,
> {
    panic!("nothing is read without a connection")
}

#[test]
fn a_paired_device_is_answered_only_with_the_chosen_connection() {
    use super::notes_chat_commands::{answer_for_paired_device, offers_paired_chat};
    use crate::application::paired_chat_exchange::{
        PairedChatOutcome, PairedChatRequest, PairedChatTurn,
    };
    use crate::domain::chat::ChatRole;

    let folder = std::env::temp_dir().join(format!("chat-paired-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    let seen: Arc<Mutex<Vec<LlmRequest>>> = Arc::default();
    let recorder = Arc::clone(&seen);
    let services = LlmServices::new(folder.join("db.sqlite3.llm-settings.json"), "Laptop")
        .with_transcription_settings(PathBuf::from("/nonexistent"))
        .with_engine_factory(Arc::new(move |_connection: &LlmConnection| {
            Ok(Box::new(Recorder(Arc::clone(&recorder))) as Box<dyn LlmEngine>)
        }));
    let repository = repository();
    let voices = InMemoryVoiceVectorDB::new();
    let meeting_id = meeting(
        &repository,
        "m-1",
        "Budget review",
        "The budget is approved.",
    );
    let request = PairedChatRequest {
        job_id: uuid::Uuid::from_u128(3),
        scope: ChatScope::Meeting(meeting_id),
        question: "Was the budget approved?".to_owned(),
        history: vec![
            PairedChatTurn {
                role: ChatRole::User,
                text: "Who came to the budget review?".to_owned(),
            },
            PairedChatTurn {
                role: ChatRole::Assistant,
                text: "Anna and Luca.".to_owned(),
            },
        ],
    };
    let gather = |scope: &ChatScope, question: &str| {
        gather_material(&repository, &voices, None, scope, question)
    };

    assert!(!offers_paired_chat(&services));
    assert_eq!(
        answer_for_paired_device(&services, &request, gather),
        PairedChatOutcome::Declined {
            error_code: "not_offered".to_owned()
        }
    );

    let saved: Value = serde_json::from_str(&run_llm_command(
        &services,
        &json!({"command": "save_connection", "label": "Local", "kind": "open_ai_compatible",
                "base_url": "http://127.0.0.1:8080/v1", "model": "m", "context_tokens": 8192})
        .to_string(),
    ))
    .unwrap_or_default();
    let id = saved["connection"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let chosen: Value = serde_json::from_str(&run_llm_command(
        &services,
        &json!({"command": "update", "paired_summary_connection": id}).to_string(),
    ))
    .unwrap_or_default();
    assert_eq!(chosen["paired_summary_connection"], id, "{chosen}");
    assert!(offers_paired_chat(&services));

    let outcome = answer_for_paired_device(&services, &request, gather);

    let PairedChatOutcome::Answered(answer) = outcome else {
        panic!("not answered: {outcome:?}");
    };
    assert_eq!(answer.text, "It was approved.");
    assert_eq!(answer.connection_label, "Local on Laptop");
    assert!(answer.included[0].starts_with("Meeting \"Budget review\""));
    let sent = seen
        .lock()
        .map(|seen| format!("{:?}", seen.last()))
        .unwrap_or_default();
    assert!(
        sent.contains("Who came to the budget review?"),
        "the history was not sent"
    );
}

#[test]
fn a_phone_keeps_the_paired_computers_answer_in_its_own_chat() {
    use super::notes_chat_commands::ask_paired_computer_with;
    use crate::application::paired_chat_exchange::{PairedChatAnswer, PairedChatTurn};
    use crate::domain::chat::{ChatMessage, ChatRole};

    let earlier = vec![
        ChatMessage::question("Who came?", chrono::Utc::now()),
        ChatMessage {
            role: ChatRole::Assistant,
            ..ChatMessage::question("Anna and Luca.", chrono::Utc::now())
        },
    ];
    let mut sent: Option<(String, Vec<PairedChatTurn>)> = None;
    let mut stored: Vec<ChatMessage> = Vec::new();
    let answer = ask_paired_computer_with(
        " Was it approved? ",
        || Ok(earlier.clone()),
        |question, turns| {
            sent = Some((question, turns));
            Ok(PairedChatAnswer {
                text: "Yes [1].".to_owned(),
                included: vec!["Meeting \"Budget review\"".to_owned()],
                omitted: Vec::new(),
                sources: Vec::new(),
                connection_label: "Claude Code on Laptop".to_owned(),
                truncated: false,
                images_sent: 0,
            })
        },
        |messages| {
            stored.extend_from_slice(messages);
            Ok(())
        },
    );

    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["answer"]["text"], "Yes [1].");
    assert_eq!(answer["answer"]["connection"], "Claude Code on Laptop");
    let (question, turns) = sent.unwrap_or_default();
    assert_eq!(question, "Was it approved?");
    assert_eq!(
        turns
            .iter()
            .map(|turn| turn.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Who came?", "Anna and Luca."]
    );
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].text, "Was it approved?");
    assert_eq!(
        stored[1].connection_label.as_deref(),
        Some("Claude Code on Laptop")
    );
}

#[test]
fn a_phone_reports_why_no_computer_answered_and_keeps_nothing() {
    use super::notes_chat_commands::ask_paired_computer_with;
    for code in ["no_paired_computer", "llm_unreachable", "rate_limited"] {
        let mut stored = 0;
        let answer = ask_paired_computer_with(
            "Approved?",
            || Ok(Vec::new()),
            |_, _| Err(code.to_owned()),
            |messages| {
                stored += messages.len();
                Ok(())
            },
        );
        assert_eq!(answer["ok"], false);
        assert_eq!(answer["error_code"], code);
        assert!(
            answer["error"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
        );
        assert_eq!(stored, 0);
    }
    let blank = ask_paired_computer_with(
        " ",
        || Ok(Vec::new()),
        |_, _| Err("x".to_owned()),
        |_| Ok(()),
    );
    assert_eq!(blank["error_code"], "empty");
}

#[test]
fn an_ask_can_name_the_paired_computer() {
    let command = parse_chat_request(
        r#"{"command":"ask","scope":"all","question":"Hi","on_paired_computer":true}"#,
    );
    assert!(matches!(
        command,
        Ok(ChatCommand::Ask {
            on_paired_computer: true,
            ..
        })
    ));
}
