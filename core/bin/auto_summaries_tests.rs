//! Specifies automatic meeting summaries (#101): which meetings are due (recorded here, recent,
//! finished, transcript final, no summary yet, not given up on) and a round that writes them with
//! the default connection only while the owner keeps automatic summaries on.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::json;

use super::{
    auto_summaries::{
        AUTO_SUMMARY_ATTEMPTS, AUTO_SUMMARY_WINDOW, AutoSummarizer, AutoSummaryHost, meetings_due,
    },
    llm_commands::{LlmServices, run_llm_command},
    meeting_summaries::PreparedSummary,
};
use crate::{
    application::{
        combined_meetings::HeldSpeech,
        meeting_summary::{SpokenLine, SummaryError, SummaryMaterial},
    },
    domain::{
        errors::LlmError,
        llm::{LlmCompletion, LlmConnection, LlmRequest, MeetingSummary},
        models::{DeviceId, MeetingId},
    },
    ports::{ai::LlmEngine, meeting_document::MeetingCombination, storage::MeetingRecording},
};

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-09T12:00:00Z")
        .map(|time| time.with_timezone(&Utc))
        .unwrap_or_default()
}

fn recording(id: &str, device: &str, hours_ago: i64) -> MeetingRecording {
    MeetingRecording {
        meeting_id: MeetingId::new(id),
        created_at: now() - TimeDelta::hours(hours_ago),
        acquisition_device_id: DeviceId::new(device),
        combined_into: None,
        kept_apart_from: Vec::new(),
        recorded_on_phone: Some(false),
        combined_with: Vec::new(),
    }
}

/// A host over fixed answers that records the summaries it stores.
#[derive(Default)]
struct FakeHost {
    recordings: Vec<MeetingRecording>,
    summarized: Vec<MeetingId>,
    recording_now: Option<MeetingId>,
    held: HashMap<MeetingId, HeldSpeech>,
    stored: Mutex<Vec<(MeetingId, String)>>,
    /// Meetings whose transcript is empty.
    empty: Vec<MeetingId>,
}

impl AutoSummaryHost for FakeHost {
    fn recordings(&self) -> Result<Vec<MeetingRecording>, String> {
        Ok(self.recordings.clone())
    }

    fn has_summary(&self, meeting_id: &MeetingId) -> Result<bool, String> {
        Ok(self.summarized.contains(meeting_id)
            || self
                .stored
                .lock()
                .is_ok_and(|stored| stored.iter().any(|(id, _)| id == meeting_id)))
    }

    fn is_recording(&self, meeting_id: &MeetingId) -> bool {
        self.recording_now.as_ref() == Some(meeting_id)
    }

    fn held_speech(&self, meeting_id: &MeetingId) -> Option<HeldSpeech> {
        self.held.get(meeting_id).copied()
    }

    fn prepare(
        &self,
        meeting_id: &MeetingId,
        _glossary: &[String],
        _device_name: &str,
    ) -> Result<PreparedSummary, SummaryError> {
        let lines = if self.empty.contains(meeting_id) {
            Vec::new()
        } else {
            vec![SpokenLine {
                speaker: "Anna".to_owned(),
                start_ms: 0,
                text: "We approve the budget.".to_owned(),
            }]
        };
        Ok(PreparedSummary {
            material: SummaryMaterial {
                lines,
                ..SummaryMaterial::default()
            },
            meeting_prompt: None,
        })
    }

    fn store(&self, meeting_id: &MeetingId, summary: &MeetingSummary) -> Result<(), SummaryError> {
        if let Ok(mut stored) = self.stored.lock() {
            stored.push((meeting_id.clone(), summary.text.clone()));
        }
        Ok(())
    }
}

impl FakeHost {
    fn stored(&self) -> Vec<(MeetingId, String)> {
        self.stored
            .lock()
            .map(|stored| stored.clone())
            .unwrap_or_default()
    }
}

/// Answers with a fixed summary, or fails when `fail` is set; counts requests.
struct FakeEngine {
    fail: bool,
    requests: Arc<Mutex<usize>>,
}

impl LlmEngine for FakeEngine {
    fn complete(&self, _request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        if let Ok(mut requests) = self.requests.lock() {
            *requests += 1;
        }
        if self.fail {
            return Err(LlmError::Unreachable);
        }
        Ok(LlmCompletion {
            text: "## Decisions\n- Budget approved.".to_owned(),
            truncated: false,
        })
    }
}

fn services(name: &str, fail: bool) -> (Arc<LlmServices>, Arc<Mutex<usize>>) {
    let folder =
        std::env::temp_dir().join(format!("auto-summaries-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("{e}"));
    let requests: Arc<Mutex<usize>> = Arc::default();
    let counter = Arc::clone(&requests);
    let services = LlmServices::new(folder.join("db.sqlite3.llm-settings.json"), "Laptop")
        .with_transcription_settings(PathBuf::from("/nonexistent"))
        .with_engine_factory(Arc::new(move |_connection: &LlmConnection| {
            Ok(Box::new(FakeEngine {
                fail,
                requests: Arc::clone(&counter),
            }) as Box<dyn LlmEngine>)
        }));
    (Arc::new(services), requests)
}

fn add_connection(services: &LlmServices) {
    let answer = run_llm_command(
        services,
        &json!({"command": "save_connection", "label": "LAN", "kind": "open_ai_compatible",
                "base_url": "http://192.168.1.5:11434/v1", "model": "llama",
                "context_tokens": 8192})
        .to_string(),
    );
    assert!(answer.contains("\"ok\":true"), "{answer}");
}

fn summarizer(services: &Arc<LlmServices>) -> AutoSummarizer {
    AutoSummarizer::new(Arc::clone(services), DeviceId::new("laptop")).with_clock(Box::new(now))
}

#[test]
fn only_finished_recent_meetings_recorded_here_without_a_summary_are_due() {
    let mut folded = recording("folded", "laptop", 1);
    folded.combined_into = Some(MeetingCombination {
        into: MeetingId::new("root"),
        offset_ms: 0,
        combine_on: DeviceId::new("laptop"),
    });
    let host = FakeHost {
        recordings: vec![
            recording("due", "laptop", 2),
            recording("other-device", "phone", 2),
            folded,
            recording(
                "old",
                "laptop",
                AUTO_SUMMARY_WINDOW.as_secs() as i64 / 3600 + 1,
            ),
            recording("recording", "laptop", 0),
            recording("waiting-for-pass", "laptop", 1),
            recording("passed", "laptop", 1),
            recording("summarized", "laptop", 1),
            recording("given-up", "laptop", 1),
        ],
        summarized: vec![MeetingId::new("summarized")],
        recording_now: Some(MeetingId::new("recording")),
        held: HashMap::from([
            (
                MeetingId::new("waiting-for-pass"),
                HeldSpeech {
                    age: Duration::from_secs(60),
                    passed_alone: false,
                },
            ),
            (
                MeetingId::new("passed"),
                HeldSpeech {
                    age: Duration::from_secs(3600),
                    passed_alone: true,
                },
            ),
        ]),
        ..FakeHost::default()
    };
    let attempts = HashMap::from([(MeetingId::new("given-up"), AUTO_SUMMARY_ATTEMPTS)]);

    let due = meetings_due(&host, &DeviceId::new("laptop"), now(), &attempts);

    assert_eq!(due, vec![MeetingId::new("due"), MeetingId::new("passed")]);
}

#[test]
fn a_round_summarizes_due_meetings_with_the_default_connection() {
    let (services, requests) = services("round", false);
    add_connection(&services);
    let host = FakeHost {
        recordings: vec![recording("m-1", "laptop", 1), recording("m-2", "laptop", 3)],
        summarized: vec![MeetingId::new("m-2")],
        ..FakeHost::default()
    };
    let mut auto = summarizer(&services);

    assert_eq!(auto.run_round(&host), 1);
    assert_eq!(
        host.stored(),
        vec![(
            MeetingId::new("m-1"),
            "## Decisions\n- Budget approved.".to_owned()
        )]
    );
    // Already summarized: the next round sends nothing.
    assert_eq!(auto.run_round(&host), 0);
    assert_eq!(*requests.lock().unwrap_or_else(|e| e.into_inner()), 1);
}

#[test]
fn nothing_is_sent_when_automatic_summaries_are_off_or_there_is_no_default() {
    let (services, requests) = services("off", false);
    let host = FakeHost {
        recordings: vec![recording("m-1", "laptop", 1)],
        ..FakeHost::default()
    };
    let mut auto = summarizer(&services);
    assert_eq!(auto.run_round(&host), 0, "no connection yet");

    add_connection(&services);
    let off = run_llm_command(
        &services,
        &json!({"command": "update", "auto_summarize": false}).to_string(),
    );
    assert!(off.contains("\"auto_summarize\":false"), "{off}");
    assert_eq!(auto.run_round(&host), 0, "turned off");
    assert!(host.stored().is_empty());
    assert_eq!(*requests.lock().unwrap_or_else(|e| e.into_inner()), 0);
}

#[test]
fn a_failing_meeting_is_given_up_after_a_few_rounds() {
    let (services, requests) = services("fail", true);
    add_connection(&services);
    let host = FakeHost {
        recordings: vec![
            recording("m-1", "laptop", 1),
            recording("empty", "laptop", 1),
        ],
        empty: vec![MeetingId::new("empty")],
        ..FakeHost::default()
    };
    let mut auto = summarizer(&services);

    for _ in 0..AUTO_SUMMARY_ATTEMPTS + 2 {
        assert_eq!(auto.run_round(&host), 0);
    }

    assert!(host.stored().is_empty());
    // Only the meeting with a transcript reached the model, once per allowed attempt.
    assert_eq!(
        *requests.lock().unwrap_or_else(|e| e.into_inner()),
        AUTO_SUMMARY_ATTEMPTS as usize
    );
}
