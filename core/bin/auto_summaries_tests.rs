//! Specifies automatic meeting summaries (#101): which meetings are due (recorded here, recent,
//! finished, transcript final, no summary yet, not given up on) and a round that writes them with
//! the default connection only while the owner keeps automatic summaries on; and summaries that
//! paired devices ask for (#107), written with the connection the owner chose for them.

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
        AUTO_SUMMARY_ATTEMPTS, AUTO_SUMMARY_WINDOW, AutoSummarizer, AutoSummaryHost,
        PAIRED_SUMMARIES_PER_HOUR, meetings_due,
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
        llm::{
            LlmCompletion, LlmConnection, LlmRequest, MeetingSummary, SummaryRequest,
            SummaryRequestState,
        },
        models::{DeviceId, MeetingId},
    },
    ports::{
        ai::LlmEngine,
        meeting_document::MeetingCombination,
        storage::{DocumentChanges, MeetingRecording},
    },
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
    /// Paired devices' summary requests (#107).
    requests: Mutex<HashMap<MeetingId, SummaryRequest>>,
    /// Summaries written for requests, which removed them.
    answered: Mutex<Vec<(MeetingId, MeetingSummary)>>,
    /// The meetings whose document changed, and the newest revision.
    changes: Mutex<(Vec<MeetingId>, u64)>,
    /// The meetings whose request was read, in order.
    request_reads: Mutex<Vec<MeetingId>>,
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

    fn summary_request(&self, meeting_id: &MeetingId) -> Result<Option<SummaryRequest>, String> {
        if let Ok(mut reads) = self.request_reads.lock() {
            reads.push(meeting_id.clone());
        }
        Ok(self
            .requests
            .lock()
            .ok()
            .and_then(|requests| requests.get(meeting_id).cloned()))
    }

    fn set_summary_request(
        &self,
        meeting_id: &MeetingId,
        request: &SummaryRequest,
    ) -> Result<(), String> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.insert(meeting_id.clone(), request.clone());
        }
        Ok(())
    }

    fn document_changes_since(&self, after_revision: u64) -> Result<DocumentChanges, String> {
        let (changed, latest_revision) = self
            .changes
            .lock()
            .map(|changes| changes.clone())
            .unwrap_or_default();
        Ok(DocumentChanges {
            meeting_ids: if after_revision < latest_revision {
                changed
            } else {
                Vec::new()
            },
            latest_revision,
        })
    }

    fn answer_request(
        &self,
        meeting_id: &MeetingId,
        summary: &MeetingSummary,
    ) -> Result<(), SummaryError> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.remove(meeting_id);
        }
        self.store(meeting_id, summary)?;
        if let Ok(mut answered) = self.answered.lock() {
            answered.push((meeting_id.clone(), summary.clone()));
        }
        Ok(())
    }
}

impl FakeHost {
    fn with_requests(mut self, requests: Vec<(&str, SummaryRequest)>) -> Self {
        self.requests = Mutex::new(
            requests
                .into_iter()
                .map(|(id, request)| (MeetingId::new(id), request))
                .collect(),
        );
        self
    }

    /// Records that these meetings' documents changed, as a sync from a phone would.
    fn change(&self, ids: &[&str]) {
        if let Ok(mut changes) = self.changes.lock() {
            changes.0 = ids.iter().map(|id| MeetingId::new(*id)).collect();
            changes.1 += 1;
        }
    }

    fn reads_and_forget(&self) -> Vec<String> {
        self.request_reads
            .lock()
            .map(|mut reads| reads.drain(..).map(|id| id.as_str().to_owned()).collect())
            .unwrap_or_default()
    }

    fn put_request(&self, id: &str, request: SummaryRequest) {
        if let Ok(mut requests) = self.requests.lock() {
            requests.insert(MeetingId::new(id), request);
        }
    }

    fn request(&self, id: &str) -> Option<SummaryRequest> {
        self.summary_request(&MeetingId::new(id)).ok().flatten()
    }

    fn answered(&self) -> Vec<(MeetingId, MeetingSummary)> {
        self.answered
            .lock()
            .map(|answered| answered.clone())
            .unwrap_or_default()
    }

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

fn phone_request(minutes_ago: i64, state: SummaryRequestState) -> SummaryRequest {
    SummaryRequest {
        requested_by: "phone".to_owned(),
        requested_at: now() - TimeDelta::minutes(minutes_ago),
        template: Some("Decisions of {{title}}.".to_owned()),
        state,
    }
}

/// Services with one connection that answers paired devices, and automatic summaries off.
fn answering_services(name: &str, fail: bool) -> (Arc<LlmServices>, Arc<Mutex<usize>>) {
    let (services, requests) = services(name, fail);
    add_connection(&services);
    let id = services.settings().connections()[0].id().to_owned();
    let answer = run_llm_command(
        &services,
        &json!({"command": "update", "auto_summarize": false, "paired_summary_connection": id})
            .to_string(),
    );
    assert!(answer.contains("\"ok\":true"), "{answer}");
    (services, requests)
}

fn sent(requests: &Arc<Mutex<usize>>) -> usize {
    *requests.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn a_paired_devices_request_is_answered_with_the_chosen_connection() {
    let (services, requests) = answering_services("answer", false);
    let host = FakeHost {
        recordings: vec![recording("p-1", "phone", 30)],
        ..FakeHost::default()
    }
    .with_requests(vec![(
        "p-1",
        phone_request(5, SummaryRequestState::Waiting),
    )]);

    assert_eq!(
        summarizer(&services).run_round(&host),
        1,
        "automatic summaries are off"
    );

    assert_eq!(host.request("p-1"), None, "the request is removed");
    let answered = host.answered();
    assert_eq!(answered.len(), 1);
    let (meeting, summary) = &answered[0];
    assert_eq!(meeting, &MeetingId::new("p-1"));
    assert_eq!(summary.text, "## Decisions\n- Budget approved.");
    assert_eq!(summary.connection_label, "LAN on Laptop");
    assert!(
        summary.prompt.starts_with("Decisions of"),
        "the asking device's template: {}",
        summary.prompt
    );
    assert_eq!(sent(&requests), 1);
}

#[test]
fn requests_wait_while_this_computer_does_not_answer_paired_devices() {
    let (services, requests) = services("not-answering", false);
    add_connection(&services);
    let host = FakeHost {
        recordings: vec![recording("p-1", "phone", 1)],
        ..FakeHost::default()
    }
    .with_requests(vec![(
        "p-1",
        phone_request(5, SummaryRequestState::Waiting),
    )]);

    summarizer(&services).run_round(&host);

    assert_eq!(
        host.request("p-1").map(|request| request.state),
        Some(SummaryRequestState::Waiting)
    );
    assert_eq!(sent(&requests), 0);
}

#[test]
fn a_failure_is_left_for_the_asking_device_and_not_retried() {
    let (services, requests) = answering_services("failure", true);
    let host = FakeHost {
        recordings: vec![recording("p-1", "phone", 1)],
        ..FakeHost::default()
    }
    .with_requests(vec![(
        "p-1",
        phone_request(5, SummaryRequestState::Waiting),
    )]);
    let mut auto = summarizer(&services);

    assert_eq!(auto.run_round(&host), 0);
    assert_eq!(
        host.request("p-1").map(|request| request.state),
        Some(SummaryRequestState::Failed {
            by: "laptop".to_owned(),
            error_code: "llm_unreachable".to_owned(),
            at: now(),
        })
    );
    auto.run_round(&host);
    assert_eq!(sent(&requests), 1, "the phone asks again if it wants to");
}

#[test]
fn another_computers_recent_work_is_left_alone_and_stale_or_own_work_is_taken_over() {
    let (services, requests) = answering_services("working", false);
    let working = |by: &str, minutes_ago: i64| SummaryRequestState::Working {
        by: by.to_owned(),
        since: now() - TimeDelta::minutes(minutes_ago),
    };
    let host = FakeHost {
        recordings: vec![
            recording("busy-elsewhere", "phone", 1),
            recording("stale-elsewhere", "phone", 1),
            recording("ours-before-restart", "phone", 1),
            recording("being-recorded", "phone", 0),
        ],
        recording_now: Some(MeetingId::new("being-recorded")),
        ..FakeHost::default()
    }
    .with_requests(vec![
        ("busy-elsewhere", phone_request(20, working("desktop", 5))),
        (
            "stale-elsewhere",
            phone_request(200, working("desktop", 120)),
        ),
        (
            "ours-before-restart",
            phone_request(30, working("laptop", 25)),
        ),
        (
            "being-recorded",
            phone_request(1, SummaryRequestState::Waiting),
        ),
    ]);

    assert_eq!(summarizer(&services).run_round(&host), 2);

    let mut answered: Vec<String> = host
        .answered()
        .into_iter()
        .map(|(id, _)| id.as_str().to_owned())
        .collect();
    answered.sort();
    assert_eq!(answered, vec!["ours-before-restart", "stale-elsewhere"]);
    assert!(host.request("busy-elsewhere").is_some());
    assert!(host.request("being-recorded").is_some());
    assert_eq!(sent(&requests), 2);
}

#[test]
fn answers_are_limited_per_hour_so_a_paired_device_cannot_use_up_the_plan() {
    let (services, requests) = answering_services("limit", false);
    let count = PAIRED_SUMMARIES_PER_HOUR + 2;
    let ids: Vec<String> = (0..count).map(|index| format!("p-{index:02}")).collect();
    let host = FakeHost {
        recordings: ids.iter().map(|id| recording(id, "phone", 1)).collect(),
        ..FakeHost::default()
    }
    .with_requests(
        ids.iter()
            .enumerate()
            .map(|(index, id)| {
                (
                    id.as_str(),
                    phone_request(60 - index as i64, SummaryRequestState::Waiting),
                )
            })
            .collect(),
    );
    let mut auto = summarizer(&services);

    assert_eq!(auto.run_round(&host), PAIRED_SUMMARIES_PER_HOUR);
    assert_eq!(auto.run_round(&host), 0, "the rest wait for the next hour");
    assert_eq!(sent(&requests), PAIRED_SUMMARIES_PER_HOUR);
    assert!(host.request("p-00").is_none(), "oldest requests first");
    assert!(host.request(&ids[count - 1]).is_some());
}

#[test]
fn a_request_with_an_invalid_template_fails_without_sending_anything() {
    let (services, requests) = answering_services("template", false);
    let mut request = phone_request(5, SummaryRequestState::Waiting);
    request.template = Some("{{nope}}".to_owned());
    let host = FakeHost {
        recordings: vec![recording("p-1", "phone", 1)],
        ..FakeHost::default()
    }
    .with_requests(vec![("p-1", request)]);

    summarizer(&services).run_round(&host);

    assert!(matches!(
        host.request("p-1").map(|request| request.state),
        Some(SummaryRequestState::Failed { error_code, .. }) if error_code == "invalid_template"
    ));
    assert_eq!(sent(&requests), 0);
}

#[test]
fn quick_checks_read_only_changed_meetings_and_requests_left_for_later() {
    let (services, requests) = answering_services("quick", false);
    let clock = Arc::new(Mutex::new(now()));
    let reading = Arc::clone(&clock);
    let mut auto = AutoSummarizer::new(Arc::clone(&services), DeviceId::new("laptop")).with_clock(
        Box::new(move || *reading.lock().unwrap_or_else(|e| e.into_inner())),
    );
    let host = FakeHost {
        recordings: vec![
            recording("quiet", "phone", 1),
            recording("asked-later", "phone", 1),
            recording("busy-elsewhere", "phone", 1),
        ],
        ..FakeHost::default()
    }
    .with_requests(vec![(
        "busy-elsewhere",
        phone_request(
            10,
            SummaryRequestState::Working {
                by: "desktop".to_owned(),
                since: now() - TimeDelta::minutes(5),
            },
        ),
    )]);

    // The first check reads every meeting, since requests may have waited while it was off.
    assert_eq!(auto.check_paired_requests(&host), 0);
    let mut first = host.reads_and_forget();
    first.sort();
    assert_eq!(first, vec!["asked-later", "busy-elsewhere", "quiet"]);

    // A request synced from the phone: only that meeting and the one left for later are read.
    host.put_request(
        "asked-later",
        phone_request(0, SummaryRequestState::Waiting),
    );
    host.change(&["asked-later"]);
    assert_eq!(auto.check_paired_requests(&host), 1);
    let mut second = host.reads_and_forget();
    second.sort();
    assert_eq!(second, vec!["asked-later", "busy-elsewhere"]);
    assert!(host.request("asked-later").is_none());

    // Nothing changed, but the other computer's work goes stale and is taken over.
    *clock.lock().unwrap_or_else(|e| e.into_inner()) = now() + TimeDelta::hours(1);
    assert_eq!(auto.check_paired_requests(&host), 1);
    assert!(!host.reads_and_forget().contains(&"quiet".to_owned()));
    assert!(host.request("busy-elsewhere").is_none());
    assert_eq!(sent(&requests), 2);
}
