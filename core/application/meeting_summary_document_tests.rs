//! Specifies how a meeting's summary and its own prompt are kept in the meeting document (#95),
//! which syncs them to paired devices.

use automerge::AutoCommit;
use chrono::{TimeZone, Utc};

use super::meeting_summary_document::{
    read_summary, read_summary_prompt, read_summary_request, with_summary, with_summary_prompt,
    with_summary_request, without_summary_request,
};
use crate::domain::llm::{MeetingSummary, SummaryRequest, SummaryRequestState};

fn summary(text: &str) -> MeetingSummary {
    MeetingSummary {
        text: text.to_owned(),
        connection_label: "Office".to_owned(),
        model: "llama".to_owned(),
        created_at: Utc
            .timestamp_millis_opt(1_762_600_000_123)
            .single()
            .unwrap_or_else(|| panic!("valid time")),
        prompt: "Summarize.".to_owned(),
        edited: false,
        truncated: true,
    }
}

fn empty() -> Vec<u8> {
    AutoCommit::new().save()
}

#[test]
fn a_summary_round_trips_and_a_new_one_replaces_it() {
    let document = empty();
    assert_eq!(read_summary(&document).ok().flatten(), None);

    let document = with_summary(&document, &summary("First")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        read_summary(&document).ok().flatten(),
        Some(summary("First"))
    );

    let mut edited = summary("Second");
    edited.edited = true;
    let document = with_summary(&document, &edited).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(read_summary(&document).ok().flatten(), Some(edited));
}

#[test]
fn a_summary_written_on_another_device_merges_in() {
    let mut here = AutoCommit::load(&empty()).unwrap_or_else(|e| panic!("{e}"));
    let there =
        with_summary(&here.save(), &summary("From the phone")).unwrap_or_else(|e| panic!("{e}"));
    let mut there = AutoCommit::load(&there).unwrap_or_else(|e| panic!("{e}"));
    here.merge(&mut there).unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(
        read_summary(&here.save())
            .ok()
            .flatten()
            .map(|summary| summary.text),
        Some("From the phone".to_owned())
    );
}

#[test]
fn the_meetings_own_prompt_is_set_and_cleared() {
    let document = with_summary_prompt(&empty(), Some("Only action items for {{title}}."))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        read_summary_prompt(&document).ok().flatten().as_deref(),
        Some("Only action items for {{title}}.")
    );
    let document = with_summary_prompt(&document, None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(read_summary_prompt(&document).ok().flatten(), None);
}

#[test]
fn a_damaged_summary_reads_as_none() {
    let mut document = AutoCommit::new();
    automerge::transaction::Transactable::put(&mut document, automerge::ROOT, "summary", 3_i64)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(read_summary(&document.save()).ok().flatten(), None);
    assert!(read_summary(b"not a document").is_err());
}

fn at(milliseconds: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_millis_opt(milliseconds)
        .single()
        .unwrap_or_else(|| panic!("valid time"))
}

fn request(state: SummaryRequestState) -> SummaryRequest {
    SummaryRequest {
        requested_by: "12D3KooWPhone".to_owned(),
        requested_at: at(1_762_600_000_000),
        template: Some("Action items of {{title}}.".to_owned()),
        state,
    }
}

#[test]
fn a_summary_request_round_trips_in_each_state_and_is_cleared() {
    let document = empty();
    assert_eq!(read_summary_request(&document).ok().flatten(), None);
    for state in [
        SummaryRequestState::Waiting,
        SummaryRequestState::Working {
            by: "12D3KooWLaptop".to_owned(),
            since: at(1_762_600_060_000),
        },
        SummaryRequestState::Failed {
            by: "12D3KooWLaptop".to_owned(),
            error_code: "llm_tool_failed".to_owned(),
            at: at(1_762_600_090_000),
        },
    ] {
        let document = with_summary_request(&document, &request(state.clone()))
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            read_summary_request(&document).ok().flatten(),
            Some(request(state))
        );
    }
    let mut without_template = request(SummaryRequestState::Waiting);
    without_template.template = None;
    let document =
        with_summary_request(&document, &without_template).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        read_summary_request(&document).ok().flatten(),
        Some(without_template)
    );
    let document = without_summary_request(&document).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(read_summary_request(&document).ok().flatten(), None);
    assert!(
        without_summary_request(&document).is_ok(),
        "clearing twice is fine"
    );
}

#[test]
fn a_request_from_the_phone_reaches_the_computer_and_the_answer_comes_back() {
    let mut phone = AutoCommit::load(&empty()).unwrap_or_else(|e| panic!("{e}"));
    let mut computer = phone.fork();
    let asked = with_summary_request(&phone.save(), &request(SummaryRequestState::Waiting))
        .unwrap_or_else(|e| panic!("{e}"));
    phone = AutoCommit::load(&asked).unwrap_or_else(|e| panic!("{e}"));
    computer.merge(&mut phone).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        read_summary_request(&computer.save()).ok().flatten(),
        Some(request(SummaryRequestState::Waiting))
    );

    let answered = with_summary(&computer.save(), &summary("From the laptop"))
        .and_then(|document| without_summary_request(&document))
        .unwrap_or_else(|e| panic!("{e}"));
    let mut computer = AutoCommit::load(&answered).unwrap_or_else(|e| panic!("{e}"));
    phone.merge(&mut computer).unwrap_or_else(|e| panic!("{e}"));
    let phone = phone.save();
    assert_eq!(read_summary_request(&phone).ok().flatten(), None);
    assert_eq!(
        read_summary(&phone)
            .ok()
            .flatten()
            .map(|summary| summary.text),
        Some("From the laptop".to_owned())
    );
}

#[test]
fn a_damaged_request_reads_as_none() {
    let mut document = AutoCommit::new();
    automerge::transaction::Transactable::put(
        &mut document,
        automerge::ROOT,
        "summary_request",
        "waiting",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(read_summary_request(&document.save()).ok().flatten(), None);
}
