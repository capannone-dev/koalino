//! Specifies a project's hand-off to the owner's own AI app for a chat (#126 follow-up): its
//! meetings as Markdown, and its own and its meetings' files copied as files, never as text.

use std::sync::Arc;

use uuid::Uuid;

use super::{project_commands::ProjectFiles, project_handoff::write_project_handoff};
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

    let written = write_project_handoff(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        project,
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
    assert_eq!(
        std::fs::read(handoff_folder.join("board.png")).unwrap_or_default(),
        [0x89, b'P', b'N', b'G']
    );
    drop(std::fs::remove_dir_all(folder));
}
