//! Specifies a project's, or every note's, hand-off to the owner's own AI app for a chat (#126
//! follow-up): the meetings as Markdown, and the files copied as files, never as text.

use std::sync::Arc;

use uuid::Uuid;

use super::{
    project_commands::ProjectFiles,
    project_handoff::{NotesHandoffScope, write_notes_handoff},
};
use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec, meeting_handoff::HandoffDelivery,
    },
    domain::{
        models::{DeviceId, MeetingId, MeetingTitle, TranscriptSegment},
        projects::ProjectName,
    },
    ports::{projects::ProjectRepository, storage::NoteRepository},
};

fn repository() -> SqliteNoteRepository {
    SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn a_project_hand_off_carries_its_meetings_and_attaches_every_file() {
    let folder = std::env::temp_dir().join(format!("project-handoff-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
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
        .add_contents(&repository, project, "budget.xlsx", b"PK-not-really")
        .unwrap_or_else(|error| panic!("{error}"));
    let meeting_id = MeetingId::new("m-1");
    MeetingCrdtService::new(&repository)
        .create_titled_meeting(
            &meeting_id,
            &MeetingTitle::parse("Kick-off").unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    repository
        .save_transcript_segments(
            &meeting_id,
            &[TranscriptSegment::new(
                meeting_id.clone(),
                0,
                1_000,
                None,
                "We start in March.",
            )],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    repository
        .move_meeting_to_project(&meeting_id, Some(project))
        .unwrap_or_else(|error| panic!("{error}"));
    let photo = folder.join("board.png");
    std::fs::write(&photo, [0x89, b'P', b'N', b'G']).unwrap_or_else(|error| panic!("{error}"));
    files
        .attach_to_meeting(&repository, &meeting_id, &photo)
        .unwrap_or_else(|error| panic!("{error}"));
    let handoff_folder = folder.join("handoff");

    let written = write_notes_handoff(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        NotesHandoffScope::Project(project),
        HandoffDelivery::Pasted,
        &handoff_folder,
    )
    .unwrap_or_else(|error| panic!("{error}"));

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
    assert_eq!(names, vec!["Launch.md", "budget.xlsx", "board.png"]);
    assert!(written.message.contains("the project \"Launch\""));
    assert!(written.message.contains("Kick-off"));
    assert!(written.message.contains("We start in March."));
    assert!(
        written
            .message
            .contains("as files: budget.xlsx, board.png.")
    );
    assert!(written.prompt.is_empty());
    assert!(!written.document_attached, "the message carries it");
    assert_eq!(
        std::fs::read(handoff_folder.join("board.png")).unwrap_or_default(),
        [0x89, b'P', b'N', b'G']
    );
    drop(std::fs::remove_dir_all(folder));
}

/// "Chat with all your notes" (#126): every meeting and every project's files. That is too long
/// to paste, so the Markdown always goes as a file and the message only introduces it.
#[test]
fn every_note_goes_as_a_file_with_a_short_message() {
    let folder = std::env::temp_dir().join(format!("all-handoff-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
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
        .add_contents(&repository, project, "budget.xlsx", b"PK-not-really")
        .unwrap_or_else(|error| panic!("{error}"));
    for (id, title, words) in [
        ("m-1", "Kick-off", "We start in March."),
        ("m-2", "Dentist", "Tuesday at nine."),
    ] {
        let meeting_id = MeetingId::new(id);
        MeetingCrdtService::new(&repository)
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
                    0,
                    1_000,
                    None,
                    words,
                )],
            )
            .unwrap_or_else(|error| panic!("{error}"));
    }
    repository
        .move_meeting_to_project(&MeetingId::new("m-1"), Some(project))
        .unwrap_or_else(|error| panic!("{error}"));
    let handoff_folder = folder.join("handoff");

    let written = write_notes_handoff(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        NotesHandoffScope::Everything,
        HandoffDelivery::Pasted,
        &handoff_folder,
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert!(written.document_attached);
    assert!(
        written.message.contains("All notes.md"),
        "{}",
        written.message
    );
    assert!(!written.message.contains("We start in March."));
    let document = std::fs::read_to_string(handoff_folder.join("All notes.md")).unwrap_or_default();
    assert!(document.contains("We start in March."));
    assert!(document.contains("Tuesday at nine."));
    assert!(document.contains("project \"Launch\""), "{document}");
    assert!(document.contains("as files: budget.xlsx."));
    assert_eq!(written.files.len(), 2);
    drop(std::fs::remove_dir_all(folder));
}
