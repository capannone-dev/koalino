//! Specifies how a meeting's summary is prepared, written and kept (#95): what is gathered from
//! the meeting, which prompt is used, and that the result and the meeting's own prompt are stored
//! in its document.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::meeting_summaries::{
    load_summary, prepare_summary, set_meeting_prompt, store_summary, summary_template_for,
};
use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::{
        llm::{DEFAULT_SUMMARY_TEMPLATE, MeetingSummary, PromptTemplate},
        models::{Attachment, MeetingId, MeetingTitle, TranscriptSegment},
    },
    ports::storage::{AttachmentRepository, NoteRepository},
};

fn repository() -> SqliteNoteRepository {
    SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"))
}

fn meeting() -> MeetingId {
    MeetingId::new("m-1")
}

/// Records a titled meeting with two segments, notes and a photo plus a PDF.
fn record(repository: &SqliteNoteRepository) -> std::path::PathBuf {
    let crdt = MeetingCrdtService::new(repository);
    let title = MeetingTitle::parse("Budget review").unwrap_or_else(|e| panic!("{e}"));
    crdt.create_titled_meeting(&meeting(), &title)
        .unwrap_or_else(|e| panic!("{e}"));
    crdt.replace_note_text(&meeting(), "Ask about Q3")
        .unwrap_or_else(|e| panic!("{e}"));
    repository
        .save_transcript_segments(
            &meeting(),
            &[
                TranscriptSegment::new(meeting(), 0, 1_000, None, "We approve it."),
                TranscriptSegment::new(meeting(), 1_000, 2_700_000, None, "Agreed."),
            ],
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let folder = std::env::temp_dir().join(format!("meeting-summaries-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("{e}"));
    let photo = folder.join("board.png");
    std::fs::write(&photo, [0x89, b'P', b'N', b'G']).unwrap_or_else(|e| panic!("{e}"));
    let pdf = folder.join("deck.pdf");
    std::fs::write(&pdf, b"%PDF").unwrap_or_else(|e| panic!("{e}"));
    for (path, mime) in [(&photo, "image/png"), (&pdf, "application/pdf")] {
        repository
            .save_attachment(&Attachment::new(
                Uuid::new_v4(),
                Some(meeting()),
                path.display().to_string(),
                mime,
                Utc::now(),
                "device",
            ))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    folder
}

#[test]
fn preparing_gathers_the_transcript_notes_photos_and_values() {
    let repository = repository();
    let folder = record(&repository);

    let prepared = prepare_summary(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        &meeting(),
        &["OKR".to_owned()],
        "Laptop",
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let material = &prepared.material;
    assert_eq!(material.values.title, "Budget review");
    assert_eq!(material.values.duration, "45 min");
    assert_eq!(material.values.device, "Laptop");
    assert_eq!(material.values.project, None);
    assert_eq!(material.values.glossary, vec!["OKR".to_owned()]);
    assert_eq!(material.notes.as_deref(), Some("Ask about Q3"));
    assert_eq!(material.lines.len(), 2);
    assert_eq!(material.lines[0].text, "We approve it.");
    assert_eq!(material.images.len(), 1, "only the photo is sent");
    assert_eq!(prepared.meeting_prompt, None);
    drop(std::fs::remove_dir_all(folder));
}

/// The text of the meeting's files goes with the transcript; files without text do not (#128).
#[test]
fn preparing_reads_the_text_of_the_meetings_files() {
    let repository = repository();
    let folder = record(&repository);
    let agenda = folder.join("agenda.md");
    std::fs::write(&agenda, "1. Budget 2027\n2. Hiring").unwrap_or_else(|e| panic!("{e}"));
    repository
        .save_attachment(&Attachment::new(
            Uuid::new_v4(),
            Some(meeting()),
            agenda.display().to_string(),
            "text/markdown",
            Utc::now(),
            "device",
        ))
        .unwrap_or_else(|e| panic!("{e}"));

    let prepared = prepare_summary(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        &meeting(),
        &[],
        "Laptop",
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let documents = &prepared.material.documents;
    assert_eq!(
        documents.len(),
        1,
        "the photo and the empty PDF give no text"
    );
    assert_eq!(documents[0].name, "agenda.md");
    assert_eq!(documents[0].text, "1. Budget 2027\n2. Hiring");
    drop(std::fs::remove_dir_all(folder));
}

#[test]
fn the_meetings_own_prompt_wins_and_a_summary_is_kept_in_its_document() {
    let repository = repository();
    let folder = record(&repository);
    assert!(set_meeting_prompt(&repository, &meeting(), Some("{{bogus}}")).is_err());
    set_meeting_prompt(
        &repository,
        &meeting(),
        Some("Only decisions in {{title}}."),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let prepared = prepare_summary(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        &meeting(),
        &[],
        "Laptop",
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let template = summary_template_for(&prepared, &PromptTemplate::default_summary());
    assert_eq!(template.as_str(), "Only decisions in {{title}}.");

    set_meeting_prompt(&repository, &meeting(), None).unwrap_or_else(|e| panic!("{e}"));
    let prepared = prepare_summary(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        &meeting(),
        &[],
        "Laptop",
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        summary_template_for(&prepared, &PromptTemplate::default_summary()).as_str(),
        DEFAULT_SUMMARY_TEMPLATE
    );

    let summary = MeetingSummary {
        text: "## Overview".to_owned(),
        connection_label: "Office".to_owned(),
        model: "llama".to_owned(),
        created_at: Utc
            .timestamp_millis_opt(1_000)
            .single()
            .unwrap_or_else(|| panic!("time")),
        prompt: "p".to_owned(),
        edited: false,
        truncated: false,
    };
    store_summary(&repository, &meeting(), &summary).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        load_summary(&repository, &meeting()).ok().flatten(),
        Some(summary)
    );
    assert!(
        repository
            .load_transcript_segments(&meeting())
            .map(|s| s.len())
            .ok()
            == Some(2),
        "the transcript is untouched"
    );
    drop(std::fs::remove_dir_all(folder));
}

#[test]
fn a_hand_off_writes_the_meeting_and_its_files_into_a_fresh_folder() {
    use crate::application::meeting_handoff::{HandoffDelivery, HandoffPurpose};
    let repository = repository();
    let folder = record(&repository);
    let handoff_folder = folder.join("handoff");
    std::fs::create_dir_all(&handoff_folder).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(handoff_folder.join("earlier.md"), "old").unwrap_or_else(|e| panic!("{e}"));

    let written = super::meeting_summaries::write_handoff(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        &meeting(),
        &[],
        "Laptop",
        None,
        &PromptTemplate::default_summary(),
        HandoffPurpose::Summary,
        HandoffDelivery::Attached,
        &handoff_folder,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert!(written.message.contains("Budget review.md"));
    assert!(!written.prompt.is_empty());
    let names: Vec<String> = written
        .files
        .iter()
        .map(|file| {
            file.path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(names, vec!["Budget review.md", "board.png", "deck.pdf"]);
    assert_eq!(written.files[0].mime_type, "text/markdown");
    assert_eq!(written.files[1].mime_type, "image/png");
    let document =
        std::fs::read_to_string(&written.files[0].path).unwrap_or_else(|e| panic!("{e}"));
    assert!(document.contains("We approve it."));
    assert!(
        document.contains("come with it as files: board.png, deck.pdf."),
        "the files are named, not turned into text (#128)"
    );
    assert_eq!(
        std::fs::read(&written.files[2].path).unwrap_or_default(),
        b"%PDF"
    );
    assert!(
        !handoff_folder.join("earlier.md").exists(),
        "an earlier hand-off's copies are removed"
    );
    drop(std::fs::remove_dir_all(folder));
}

/// A computer's P2P status, paired with the phone that recorded the test meeting (#119).
fn computer_paired_with_the_recording_phone() -> crate::application::p2p_service::P2pStatus {
    use crate::domain::{
        models::{DeviceId, DeviceManifest},
        peer_trust::{DeviceName, TrustedPeer},
    };
    let name = |name: &str| DeviceName::parse(name).unwrap_or_else(|error| panic!("{error}"));
    let mut local_manifest = DeviceManifest::new(DeviceId::new("peer-computer"), 4, 0, Vec::new());
    local_manifest.storage_device_id = Some(DeviceId::new("computer"));
    crate::application::p2p_service::P2pStatus {
        local_peer_id: DeviceId::new("peer-computer"),
        device_name: name("Computer"),
        transcription_route: crate::application::compute_router::ComputePreference::LocalOnly,
        trusted_devices: vec![
            TrustedPeer::new(DeviceId::new("peer-phone"), name("Galaxy"), 1)
                .with_storage_device_id(Some(DeviceId::new("in-memory-test-device"))),
        ],
        nearby_devices: Vec::new(),
        nearby_device_names: Vec::new(),
        incoming_requests: Vec::new(),
        outgoing_pairings: Vec::new(),
        recent_updates: Vec::new(),
        local_manifest,
        peer_manifests: Vec::new(),
        connected_devices: Vec::new(),
    }
}

#[test]
fn the_device_is_the_one_that_recorded_the_meeting_not_the_one_summarizing() {
    let repository = repository();
    let folder = record(&repository);
    let status = computer_paired_with_the_recording_phone();

    let prepared = prepare_summary(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        &meeting(),
        &[],
        "Computer",
        Some(&status),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(prepared.material.values.device, "Galaxy");
    drop(std::fs::remove_dir_all(folder));
}

#[test]
fn the_duration_is_rounded_to_the_nearest_minute_and_is_never_zero() {
    let duration_for = |end_ms: u64| {
        let repository = repository();
        MeetingCrdtService::new(&repository)
            .create_titled_meeting(
                &meeting(),
                &MeetingTitle::parse("Short").unwrap_or_else(|e| panic!("{e}")),
            )
            .unwrap_or_else(|e| panic!("{e}"));
        repository
            .save_transcript_segments(
                &meeting(),
                &[TranscriptSegment::new(meeting(), 0, end_ms, None, "Hi.")],
            )
            .unwrap_or_else(|e| panic!("{e}"));
        prepare_summary(
            &repository,
            &InMemoryVoiceVectorDB::new(),
            &meeting(),
            &[],
            "Laptop",
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"))
        .material
        .values
        .duration
    };

    assert_eq!(duration_for(600_000), "10 min", "10:00 is ten minutes");
    assert_eq!(duration_for(629_999), "10 min");
    assert_eq!(duration_for(630_000), "11 min");
    assert_eq!(duration_for(5_000), "1 min");
}
