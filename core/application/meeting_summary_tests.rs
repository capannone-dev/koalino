//! Specifies how a meeting summary is asked for (#95): what the model is sent, how a long
//! transcript is condensed in parts instead of cut, how images are sent or skipped, and what the
//! stored summary records.

use std::sync::Mutex;

use chrono::{TimeZone, Utc};

use super::meeting_summary::{
    SpokenLine, SummaryError, SummaryMaterial, render_transcript, summarize_meeting,
};
use crate::{
    domain::{
        errors::LlmError,
        llm::{
            LlmCompletion, LlmConnection, LlmImage, LlmKind, LlmRequest, PromptTemplate,
            PromptValues,
        },
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
        "llama",
        context_tokens,
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

fn material(lines: Vec<SpokenLine>) -> SummaryMaterial {
    SummaryMaterial {
        values: PromptValues {
            title: "Budget review".to_owned(),
            date: "2026-11-08".to_owned(),
            time: "10:00".to_owned(),
            duration: "45 min".to_owned(),
            device: "Laptop".to_owned(),
            project: Some("Hiring".to_owned()),
            speakers: vec!["Anna".to_owned(), "Marco".to_owned()],
            glossary: vec!["OKR".to_owned()],
            language: "it".to_owned(),
        },
        notes: Some("Ask about Q3".to_owned()),
        lines,
        images: LlmImage::new("image/jpeg", vec![9; 10])
            .into_iter()
            .collect(),
    }
}

fn line(speaker: &str, start_ms: u64, text: &str) -> SpokenLine {
    SpokenLine {
        speaker: speaker.to_owned(),
        start_ms,
        text: text.to_owned(),
    }
}

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 8, 12, 0, 0)
        .single()
        .unwrap_or_else(|| panic!("valid time"))
}

#[test]
fn the_transcript_joins_a_speakers_consecutive_lines_and_shows_times() {
    let text = render_transcript(&[
        line("Anna", 0, "Hello."),
        line("Anna", 2_000, "Shall we start?"),
        line("Marco", 3_725_000, "Yes."),
    ]);
    assert_eq!(
        text,
        "[00:00:00] Anna: Hello. Shall we start?\n[01:02:05] Marco: Yes."
    );
}

#[test]
fn a_long_turn_gets_a_new_time_every_half_minute_so_answers_can_cite_moments() {
    let text = render_transcript(&[
        line("Speaker 1", 0, "One."),
        line("Speaker 1", 10_000, "Two."),
        line("Speaker 1", 40_000, "Three."),
        line("Speaker 1", 75_000, "Four."),
    ]);
    assert_eq!(
        text,
        "[00:00:00] Speaker 1: One. Two.\n[00:00:40] Speaker 1: Three.\n[00:01:15] Speaker 1: Four."
    );
}

#[test]
fn a_short_meeting_is_summarized_in_one_request_with_the_rendered_template_and_images() {
    let engine = ScriptedEngine::new(vec![answer("## Overview\nAll good.")]);
    let template = PromptTemplate::parse("Summarize {{title}} for {{project}} in {{language}}.")
        .unwrap_or_else(|e| panic!("{e}"));

    let outcome = summarize_meeting(
        &engine,
        &connection(32_768),
        &template,
        material(vec![line("Anna", 0, "We approve the budget.")]),
        now(),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let seen = engine.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].system, "Summarize Budget review for Hiring in it.");
    assert_eq!(seen[0].turns.len(), 1);
    let user = &seen[0].turns[0].text;
    assert!(user.contains("Ask about Q3"), "the owner's notes are sent");
    assert!(user.contains("[00:00:00] Anna: We approve the budget."));
    assert_eq!(seen[0].images.len(), 1);
    assert_eq!(outcome.summary.text, "## Overview\nAll good.");
    assert_eq!(
        outcome.summary.prompt,
        "Summarize Budget review for Hiring in it."
    );
    assert_eq!(outcome.summary.model, "llama");
    assert_eq!(outcome.summary.connection_label, "Office");
    assert_eq!(outcome.summary.created_at, now());
    assert!(!outcome.summary.edited);
    assert_eq!(
        (outcome.parts, outcome.images_sent, outcome.images_skipped),
        (1, 1, 0)
    );
}

#[test]
fn a_transcript_too_long_for_the_context_is_condensed_in_parts_never_cut() {
    let long_lines: Vec<SpokenLine> = (0..400_u64)
        .map(|index| {
            line(
                if index % 2 == 0 { "Anna" } else { "Marco" },
                index * 10_000,
                &format!(
                    "Point number {index} about the budget and the hiring plan for next year."
                ),
            )
        })
        .collect();
    let replies: Vec<_> = (0..20)
        .map(|part| answer(&format!("notes {part}")))
        .collect();
    let engine = ScriptedEngine::new(replies);

    let outcome = summarize_meeting(
        &engine,
        &connection(4_096),
        &PromptTemplate::default_summary(),
        material(long_lines),
        now(),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let seen = engine.seen();
    assert!(outcome.parts > 1, "split into parts");
    assert_eq!(
        seen.len(),
        outcome.parts + 1,
        "one request per part, then the summary"
    );
    let everything: String = seen[..outcome.parts]
        .iter()
        .map(|request| request.turns[0].text.clone())
        .collect();
    for index in [0, 199, 399] {
        assert!(
            everything.contains(&format!("Point number {index} ")),
            "line {index} sent"
        );
    }
    assert!(
        seen[..outcome.parts]
            .iter()
            .all(|request| request.images.is_empty())
    );
    let last = &seen[outcome.parts];
    assert!(last.turns[0].text.contains("notes 0"));
    assert_eq!(last.images.len(), 1, "images go with the final request");
    let budget_chars = 4_096 * 3;
    assert!(
        seen.iter()
            .all(|request| request.turns[0].text.len() + request.system.len() < budget_chars)
    );
}

#[test]
fn a_server_that_cannot_read_images_is_asked_again_without_them() {
    let engine = ScriptedEngine::new(vec![
        Err(LlmError::Rejected {
            status: 400,
            provider_reason: None,
        }),
        answer("Done"),
    ]);

    let outcome = summarize_meeting(
        &engine,
        &connection(32_768),
        &PromptTemplate::default_summary(),
        material(vec![line("Anna", 0, "Text.")]),
        now(),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let seen = engine.seen();
    assert_eq!(seen.len(), 2);
    assert!(seen[1].images.is_empty());
    assert_eq!((outcome.images_sent, outcome.images_skipped), (0, 1));
}

/// Answers like Claude Code run without tools (#105): it cannot read images.
struct TextOnlyEngine(ScriptedEngine);

impl LlmEngine for TextOnlyEngine {
    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        self.0.complete(request)
    }

    fn reads_images(&self) -> bool {
        false
    }
}

#[test]
fn an_engine_that_cannot_read_images_gets_none_and_they_are_counted_as_left_out() {
    let engine = TextOnlyEngine(ScriptedEngine::new(vec![answer("Done")]));

    let outcome = summarize_meeting(
        &engine,
        &connection(32_768),
        &PromptTemplate::default_summary(),
        material(vec![line("Anna", 0, "Text.")]),
        now(),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let seen = engine.0.seen();
    assert_eq!(seen.len(), 1, "no retry is needed");
    assert!(seen[0].images.is_empty());
    assert_eq!((outcome.images_sent, outcome.images_skipped), (0, 1));
}

#[test]
fn nothing_said_or_written_is_not_sent_and_failures_are_reported() {
    let engine = ScriptedEngine::new(vec![]);
    let mut empty = material(Vec::new());
    empty.notes = Some("   ".to_owned());
    assert!(matches!(
        summarize_meeting(
            &engine,
            &connection(8_192),
            &PromptTemplate::default_summary(),
            empty,
            now()
        ),
        Err(SummaryError::NothingToSummarize)
    ));
    assert!(engine.seen().is_empty());

    let refusing = ScriptedEngine::new(vec![Err(LlmError::Unauthorized)]);
    assert!(matches!(
        summarize_meeting(
            &refusing,
            &connection(8_192),
            &PromptTemplate::default_summary(),
            material(vec![line("Anna", 0, "Text.")]),
            now()
        ),
        Err(SummaryError::Llm(LlmError::Unauthorized))
    ));
}
