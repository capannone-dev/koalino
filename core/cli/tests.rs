//! Defines parser and service-dispatch contracts before the CLI adapter implementation.

use std::{cell::RefCell, str::FromStr};

use clap::Parser;
use uuid::Uuid;

use super::{
    Cli, CliCommand, CliService, CoreCliService, MeetingArgs, MeetingCommand, RecordArgs,
    RecordCommand, VoiceArgs, VoiceCommand, dispatch,
};
use crate::{
    adapters::{
        audio::local_recorder::LocalRecorder,
        storage::{
            in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
        },
    },
    domain::models::{AudioConfig, MeetingId},
    ports::storage::NoteRepository,
};

/// Records calls made by the mock service without using an external database or service.
#[derive(Default)]
struct MockCliService {
    assigned_clips: RefCell<Vec<(Uuid, Uuid)>>,
}

impl CliService for MockCliService {
    fn start_recording(&mut self) -> Result<String, String> {
        Ok(String::from("recording started"))
    }
    fn stop_recording(&mut self) -> Result<String, String> {
        Ok(String::from("recording stopped"))
    }
    fn recording_status(&self) -> Result<String, String> {
        Ok(String::from("idle"))
    }
    fn list_meetings(&self) -> Result<Vec<(String, String, String)>, String> {
        Ok(vec![(
            String::from("meeting-rome"),
            String::from("2026-10-25"),
            String::from("unknown"),
        )])
    }
    fn view_meeting(&self, _meeting_id: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn list_voices(&self) -> Result<Vec<(String, String, usize)>, String> {
        Ok(Vec::new())
    }
    fn assign_voice_clip(&mut self, clip_id: Uuid, identity_id: Uuid) -> Result<String, String> {
        self.assigned_clips
            .borrow_mut()
            .push((clip_id, identity_id));
        Ok(String::from("voice clip assigned"))
    }
    fn telemetry(&self) -> Result<String, String> {
        Ok(String::new())
    }
}

/// Verifies clap derives parse the nested recording command without a process exit.
#[test]
fn parses_record_start_command() {
    let parsed_cli = Cli::try_parse_from(["note-taker", "record", "start"])
        .unwrap_or_else(|error| panic!("record start must parse: {error}"));
    assert!(matches!(
        parsed_cli.command,
        CliCommand::Record(RecordArgs {
            command: RecordCommand::Start
        })
    ));
}

/// Verifies the CLI requires both stable UUID arguments when manually resolving a voice clip.
#[test]
fn parses_voice_assign_command_with_stable_identifiers() {
    let clip_id = Uuid::new_v4();
    let identity_id = Uuid::new_v4();
    let parsed_cli = Cli::try_parse_from([
        "note-taker",
        "voice",
        "assign",
        &clip_id.to_string(),
        &identity_id.to_string(),
    ])
    .unwrap_or_else(|error| panic!("voice assign must parse: {error}"));

    assert!(
        matches!(parsed_cli.command, CliCommand::Voice(VoiceArgs { command: VoiceCommand::Assign { clip_id: parsed_clip_id, identity_id: parsed_identity_id } }) if parsed_clip_id == clip_id && parsed_identity_id == identity_id)
    );
}

/// Verifies malformed UUID input is rejected by clap before the service layer can be called.
#[test]
fn rejects_malformed_voice_assignment_identifier() {
    assert!(
        Cli::try_parse_from([
            "note-taker",
            "voice",
            "assign",
            "not-a-uuid",
            "also-not-a-uuid"
        ])
        .is_err()
    );
}

/// Verifies the list command delegates formatting input to the injected service rather than SQLite.
#[test]
fn meetings_list_formats_mocked_service_rows_as_a_table() {
    let mut mock_service = MockCliService::default();
    let output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::List,
        }),
        &mut mock_service,
    )
    .unwrap_or_else(|error| panic!("meeting list must dispatch: {error}"));

    assert!(output.contains("ID | Date | Duration"));
    assert!(output.contains("meeting-rome | 2026-10-25 | unknown"));
}

/// Verifies voice assignment maps parsed UUIDs directly to the injected application service.
#[test]
fn voice_assign_delegates_to_the_service_once() {
    let clip_id = Uuid::from_str("00000000-0000-4000-8000-000000000001")
        .unwrap_or_else(|error| panic!("fixture UUID must parse: {error}"));
    let identity_id = Uuid::from_str("00000000-0000-4000-8000-000000000002")
        .unwrap_or_else(|error| panic!("fixture UUID must parse: {error}"));
    let mut mock_service = MockCliService::default();

    let output = dispatch(
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::Assign {
                clip_id,
                identity_id,
            },
        }),
        &mut mock_service,
    )
    .unwrap_or_else(|error| panic!("voice assignment must dispatch: {error}"));

    assert_eq!(output, "voice clip assigned");
    assert_eq!(
        mock_service.assigned_clips.into_inner(),
        vec![(clip_id, identity_id)]
    );
}

/// Verifies the concrete CLI service reads a meeting fixture through in-memory SQLite ports.
#[test]
fn meetings_list_reads_an_in_memory_sqlite_fixture_without_cli_sql() {
    let repository = SqliteNoteRepository::open_in_memory()
        .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-in-memory");
    repository
        .create_meeting(&meeting_id, &[1, 2, 3])
        .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
    let recorder = LocalRecorder::new(AudioConfig::new(16_000, 1))
        .unwrap_or_else(|error| panic!("fixture recorder must initialize: {error}"));
    let mut cli_service = CoreCliService::new(recorder, repository, InMemoryVoiceVectorDB::new());

    let output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::List,
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("meeting list must dispatch: {error}"));

    assert!(output.contains("meeting-in-memory"));
}
