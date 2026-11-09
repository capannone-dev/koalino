//! Specifies how a meeting's summary and its own prompt are kept in the meeting document (#95),
//! which syncs them to paired devices.

use automerge::AutoCommit;
use chrono::{TimeZone, Utc};

use super::meeting_summary_document::{
    read_summary, read_summary_prompt, with_summary, with_summary_prompt,
};
use crate::domain::llm::MeetingSummary;

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
