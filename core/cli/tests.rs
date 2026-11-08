//! Defines parser and service-dispatch contracts before the CLI adapter implementation.

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use uuid::Uuid;

use super::{
    Cli, CliCommand, CliService, CoreCliService, MeetingArgs, MeetingCommand, MeetingListRow,
    PairedDeviceRow, RecordArgs, RecordCommand, RecordingControl, VoiceArgs, VoiceCommand,
    dispatch,
};
use crate::{
    adapters::{
        export::default_exporters,
        observability::LogLevelControlFile,
        storage::{
            in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
            sqlite_peer_store::SqlitePeerStore, sqlite_voice_vector_db::SqliteVoiceVectorDB,
        },
    },
    application::export_manager::ExportWriter,
    application::unknown_speaker_clusterer::{ClusteringConfig, UnknownSpeakerClusterer},
    domain::models::{
        DeviceId, ExportFormat, ExportOptions, MeetingId, MeetingTitle, TranscriptSegment,
    },
    domain::peer_trust::{DeviceName, TrustedPeer},
    domain::transcription_language::TranscriptionLanguage,
    ports::{peer_trust::TrustedPeerStore, storage::NoteRepository},
};

/// One recorded export call: optional meeting ID (`None` for export-all), options, and output.
type RecordedExportRequest = (Option<String>, ExportOptions, Option<PathBuf>);

/// Records calls made by the mock service without using an external database or service.
#[derive(Default)]
struct MockCliService {
    assigned_clips: RefCell<Vec<(Uuid, Uuid)>>,
    /// Rows returned by `list_unknown_clusters`.
    cluster_rows: Vec<(String, u64, usize)>,
    /// Meeting IDs passed to `list_unknown_clusters`.
    listed_cluster_meetings: RefCell<Vec<String>>,
    /// `(cluster_id, identity_name)` pairs passed to `assign_cluster`.
    assigned_clusters: RefCell<Vec<(Uuid, String)>>,
    export_requests: RefCell<Vec<RecordedExportRequest>>,
    /// Arguments passed to `start_recording`.
    recording_requests: Vec<RecordingRequest>,
    /// `(meeting_id, new_title)` pairs passed to `rename_meeting`.
    rename_requests: Vec<(String, String)>,
    /// Directives passed to `request_log_level`.
    log_level_requests: Vec<String>,
    /// Rows returned by `list_paired_devices`.
    paired_device_rows: Vec<PairedDeviceRow>,
    /// Selectors passed to `revoke_device`.
    revoked_devices: Vec<String>,
    /// The glossary as saved through the service.
    glossary: Vec<String>,
}

/// The arguments of one `start_recording` call.
#[derive(Debug, PartialEq, Eq)]
struct RecordingRequest {
    meeting_id: Option<String>,
    title: Option<String>,
    input: Option<PathBuf>,
    language: Option<String>,
}

/// Rejects every recording command; for service tests that never record.
struct NoRecordingControl;

impl RecordingControl for NoRecordingControl {
    fn record(
        &mut self,
        _meeting_id: Option<MeetingId>,
        _title: Option<MeetingTitle>,
        _input: Option<&Path>,
        _language: Option<TranscriptionLanguage>,
    ) -> Result<String, String> {
        Err(String::from("recording is not available in this test"))
    }
    fn request_stop(&mut self) -> Result<String, String> {
        Err(String::from("no recording is active"))
    }
    fn status(&self) -> Result<String, String> {
        Ok(String::from("Idle"))
    }
}

impl CliService for MockCliService {
    fn delete_meeting(&mut self, _meeting_id: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn split_meeting(&mut self, _meeting_id: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn delete_unknown_voice(&mut self, _cluster_id: Uuid) -> Result<String, String> {
        Ok(String::new())
    }
    fn delete_saved_person(&mut self, _identity_id: Uuid) -> Result<String, String> {
        Ok(String::new())
    }
    fn start_recording(
        &mut self,
        meeting_id: Option<&str>,
        title: Option<&str>,
        input: Option<&Path>,
        language: Option<&str>,
    ) -> Result<String, String> {
        self.recording_requests.push(RecordingRequest {
            meeting_id: meeting_id.map(str::to_owned),
            title: title.map(str::to_owned),
            input: input.map(Path::to_path_buf),
            language: language.map(str::to_owned),
        });
        Ok(String::from("recording started"))
    }
    fn stop_recording(&mut self) -> Result<String, String> {
        Ok(String::from("recording stopped"))
    }
    fn recording_status(&self) -> Result<String, String> {
        Ok(String::from("idle"))
    }
    fn list_meetings(&self) -> Result<Vec<MeetingListRow>, String> {
        Ok(vec![MeetingListRow {
            meeting_id: String::from("meeting-rome"),
            title: String::from("Rome offsite"),
            created_at: String::from("2026-10-25 09:00 UTC"),
            duration: String::from("unknown"),
            device_count: 1,
        }])
    }
    fn view_meeting(&self, _meeting_id: &str) -> Result<String, String> {
        Ok(String::new())
    }
    fn rename_meeting(&mut self, meeting_id: &str, new_title: &str) -> Result<String, String> {
        self.rename_requests
            .push((String::from(meeting_id), String::from(new_title)));
        Ok(String::from("meeting renamed"))
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
    fn list_unknown_clusters(&self, meeting_id: &str) -> Result<Vec<(String, u64, usize)>, String> {
        self.listed_cluster_meetings
            .borrow_mut()
            .push(String::from(meeting_id));
        Ok(self.cluster_rows.clone())
    }
    fn assign_cluster(&mut self, cluster_id: Uuid, identity_name: &str) -> Result<String, String> {
        self.assigned_clusters
            .borrow_mut()
            .push((cluster_id, String::from(identity_name)));
        Ok(String::from("cluster assigned"))
    }
    fn telemetry(&self) -> Result<String, String> {
        Ok(String::new())
    }
    fn request_log_level(&mut self, filter_directive: &str) -> Result<String, String> {
        self.log_level_requests.push(filter_directive.to_owned());
        Ok(String::from("log level requested"))
    }
    fn list_paired_devices(&self) -> Result<Vec<PairedDeviceRow>, String> {
        Ok(self.paired_device_rows.clone())
    }
    fn revoke_device(&mut self, device: &str) -> Result<String, String> {
        self.revoked_devices.push(device.to_owned());
        Ok(String::from("device removed"))
    }
    fn glossary(&self) -> Result<Vec<String>, String> {
        Ok(self.glossary.clone())
    }
    fn save_glossary(&mut self, terms: &[String]) -> Result<(), String> {
        self.glossary = terms.to_vec();
        Ok(())
    }
    fn export_meeting(
        &self,
        meeting_id: &str,
        options: &ExportOptions,
        output: Option<&Path>,
    ) -> Result<String, String> {
        self.export_requests.borrow_mut().push((
            Some(String::from(meeting_id)),
            options.clone(),
            output.map(Path::to_path_buf),
        ));
        Ok(String::from("exported"))
    }
    fn export_all(&self, options: &ExportOptions, output: Option<&Path>) -> Result<String, String> {
        self.export_requests.borrow_mut().push((
            None,
            options.clone(),
            output.map(Path::to_path_buf),
        ));
        Ok(String::from("exported all"))
    }
}

/// Verifies clap derives parse the nested recording command without a process exit.
#[test]
fn parses_record_start_command() {
    let parsed_cli = Cli::try_parse_from([
        "note-taker",
        "record",
        "start",
        "--input",
        "standup.wav",
        "--meeting-id",
        "standup",
        "--title",
        "Daily standup",
        "--language",
        "it",
    ])
    .unwrap_or_else(|error| panic!("record start must parse: {error}"));
    match parsed_cli.command {
        CliCommand::Record(RecordArgs {
            command:
                RecordCommand::Start {
                    meeting_id,
                    title,
                    input,
                    language,
                },
        }) => {
            assert_eq!(meeting_id.as_deref(), Some("standup"));
            assert_eq!(title.as_deref(), Some("Daily standup"));
            assert_eq!(input, Some(PathBuf::from("standup.wav")));
            assert_eq!(language.as_deref(), Some("it"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

/// Verifies `record start` without `--input` records the microphone (Issue #45).
#[test]
fn record_start_defaults_to_the_microphone() {
    let parsed_cli = Cli::try_parse_from(["note-taker", "record", "start"])
        .unwrap_or_else(|error| panic!("record start must parse: {error}"));
    match parsed_cli.command {
        CliCommand::Record(RecordArgs {
            command: RecordCommand::Start { input, .. },
        }) => assert_eq!(input, None),
        other => panic!("unexpected command: {other:?}"),
    }
}

/// Verifies `record start` forwards its meeting, optional title, input and language to the
/// service.
#[test]
fn dispatches_record_start_with_meeting_title_and_input() {
    let mut cli_service = MockCliService::default();
    for arguments in [
        vec!["note-taker", "record", "start", "--input", "-"],
        vec![
            "note-taker",
            "record",
            "start",
            "--language",
            "en",
            "--input",
            "-",
        ],
        vec![
            "note-taker",
            "record",
            "start",
            "--title",
            "Test",
            "--input",
            "-",
        ],
    ] {
        let parsed_cli = Cli::try_parse_from(arguments)
            .unwrap_or_else(|error| panic!("record start must parse: {error}"));
        let output = dispatch(parsed_cli.command, &mut cli_service)
            .unwrap_or_else(|error| panic!("record start must dispatch: {error}"));
        assert_eq!(output, "recording started");
    }

    assert_eq!(
        cli_service.recording_requests,
        vec![
            RecordingRequest {
                meeting_id: None,
                title: None,
                input: Some(PathBuf::from("-")),
                language: None,
            },
            RecordingRequest {
                meeting_id: None,
                title: None,
                input: Some(PathBuf::from("-")),
                language: Some(String::from("en")),
            },
            RecordingRequest {
                meeting_id: None,
                title: Some(String::from("Test")),
                input: Some(PathBuf::from("-")),
                language: None,
            },
        ]
    );
}

/// Verifies the core service rejects an invalid title before any recording starts.
#[test]
fn core_service_rejects_an_invalid_recording_title() {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    let mut cli_service = CoreCliService::new(
        NoRecordingControl,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), std::env::temp_dir()),
    );

    let error = cli_service
        .start_recording(None, Some("  "), Some(Path::new("-")), None)
        .expect_err("a blank title must be rejected");

    assert_eq!(error, "--title: meeting title must not be blank");
}

/// Verifies the core service refuses a language Whisper does not know before any recording.
#[test]
fn core_service_rejects_an_unknown_recording_language() {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    let mut cli_service = CoreCliService::new(
        NoRecordingControl,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), std::env::temp_dir()),
    );

    let error = cli_service
        .start_recording(None, None, Some(Path::new("-")), Some("klingon"))
        .expect_err("an unknown language must be rejected");

    assert!(error.starts_with("--language:"), "{error}");
    assert!(error.contains("auto"), "{error}");
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

    assert!(output.contains("ID | Title | Date | Duration"));
    assert!(output.contains("meeting-rome | Rome offsite | 2026-10-25 09:00 UTC | unknown"));
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
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-in-memory");
    repository
        .create_meeting(&meeting_id, &[1, 2, 3])
        .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
    let mut cli_service = CoreCliService::new(
        NoRecordingControl,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), std::env::temp_dir()),
    );

    let output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::List,
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("meeting list must dispatch: {error}"));

    assert!(output.contains("meeting-in-memory"));
}

/// Verifies `meetings list` shows stored titles, dated fallbacks, creation dates, and durations,
/// and `meetings view` puts the title above everything else.
#[test]
fn core_service_lists_and_views_meeting_titles() {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    let titled_meeting_id = MeetingId::new("meeting-titled");
    let untitled_meeting_id = MeetingId::new("meeting-untitled");
    for meeting_id in [&titled_meeting_id, &untitled_meeting_id] {
        repository
            .create_meeting(meeting_id, &[1])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
    }
    repository
        .save_meeting_title(
            &titled_meeting_id,
            &MeetingTitle::parse("Project Sync")
                .unwrap_or_else(|error| panic!("title must parse: {error}")),
        )
        .unwrap_or_else(|error| panic!("title must persist: {error}"));
    repository
        .save_transcript_segments(
            &titled_meeting_id,
            &[TranscriptSegment::new(
                titled_meeting_id.clone(),
                0,
                65_000,
                None,
                "Kick-off.",
            )],
        )
        .unwrap_or_else(|error| panic!("segments must persist: {error}"));
    let created_date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut cli_service = CoreCliService::new(
        NoRecordingControl,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), std::env::temp_dir()),
    );

    let list_output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::List,
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("meeting list must dispatch: {error}"));
    let rows: Vec<Vec<&str>> = list_output
        .lines()
        .skip(1)
        .map(|line| line.split(" | ").collect())
        .collect();
    let titled_row = rows
        .iter()
        .find(|row| row[0] == "meeting-titled")
        .unwrap_or_else(|| panic!("titled meeting must be listed: {list_output}"));
    let untitled_row = rows
        .iter()
        .find(|row| row[0] == "meeting-untitled")
        .unwrap_or_else(|| panic!("untitled meeting must be listed: {list_output}"));
    assert_eq!(titled_row[1], "Project Sync");
    assert!(titled_row[2].starts_with(&created_date), "{list_output}");
    assert_eq!(titled_row[3], "1:05");
    assert!(
        untitled_row[1].starts_with(&format!("Meeting {created_date}")),
        "{list_output}"
    );
    assert_eq!(untitled_row[3], "0:00");

    let view_output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::View {
                meeting_id: String::from("meeting-titled"),
            },
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("meeting view must dispatch: {error}"));
    let mut view_lines = view_output.lines();
    assert_eq!(view_lines.next(), Some("Project Sync"));
    assert_eq!(view_lines.next(), Some("============"));
    assert_eq!(view_lines.next(), Some("Meeting: meeting-titled"));
    // A meeting with no recorded language has no language line.
    assert!(
        !view_output.contains("Transcription language"),
        "{view_output}"
    );

    let missing_error = cli_service
        .view_meeting("meeting-missing")
        .expect_err("an unknown meeting must be reported");
    assert_eq!(missing_error, "meeting meeting-missing does not exist");
}

/// Parses `meetings export` arguments through the shared clap definition.
fn parse_export_command(arguments: &[&str]) -> Result<MeetingCommand, clap::Error> {
    let mut full_arguments = vec!["note-taker", "meetings"];
    full_arguments.extend_from_slice(arguments);
    Cli::try_parse_from(full_arguments).and_then(|parsed_cli| match parsed_cli.command {
        CliCommand::Meetings(MeetingArgs { command }) => Ok(command),
        _ => Err(clap::Error::new(clap::error::ErrorKind::InvalidSubcommand)),
    })
}

/// Verifies `meetings export` parses the meeting, format, destination, and inclusion flags.
#[test]
fn parses_meetings_export_with_format_output_and_flags() {
    let parsed_command = parse_export_command(&[
        "export",
        "meeting-rome",
        "--format",
        "PDF",
        "--output",
        "/tmp/minutes.pdf",
        "--no-attachments",
    ])
    .unwrap_or_else(|error| panic!("meetings export must parse: {error}"));

    let MeetingCommand::Export {
        meeting_id,
        export_arguments,
    } = parsed_command
    else {
        panic!("expected the export subcommand");
    };
    assert_eq!(meeting_id, "meeting-rome");
    assert_eq!(
        export_arguments.output,
        Some(PathBuf::from("/tmp/minutes.pdf"))
    );
    assert_eq!(
        export_arguments.export_options(None),
        ExportOptions::new(ExportFormat::Pdf, false, true, None)
    );
}

/// Verifies `meetings export-all` parses an inclusive day range covering whole UTC days.
#[test]
fn parses_meetings_export_all_with_inclusive_date_range() {
    let parsed_command = parse_export_command(&[
        "export-all",
        "--format",
        "json",
        "--date-range",
        "2026-10-01:2026-10-31",
    ])
    .unwrap_or_else(|error| panic!("meetings export-all must parse: {error}"));

    let MeetingCommand::ExportAll {
        date_range: Some(date_range),
        export_arguments,
    } = parsed_command
    else {
        panic!("expected export-all with a date range");
    };
    assert_eq!(export_arguments.format, ExportFormat::Json);
    assert_eq!(date_range.start().to_rfc3339(), "2026-10-01T00:00:00+00:00");
    assert_eq!(
        date_range.end().to_rfc3339(),
        "2026-10-31T23:59:59.999+00:00"
    );
}

/// Verifies malformed formats and date ranges are rejected before reaching the service.
#[test]
fn rejects_unknown_export_format_and_invalid_date_ranges() {
    assert!(parse_export_command(&["export", "meeting-rome", "--format", "docx"]).is_err());
    assert!(parse_export_command(&["export", "meeting-rome"]).is_err());
    for invalid_range in ["2026-10-31:2026-10-01", "2026-10-01", "yesterday:today"] {
        assert!(
            parse_export_command(&[
                "export-all",
                "--format",
                "json",
                "--date-range",
                invalid_range
            ])
            .is_err(),
            "{invalid_range} must be rejected"
        );
    }
}

/// Verifies export commands map flags to `ExportOptions` and forward them to the service.
#[test]
fn meetings_export_commands_delegate_options_to_the_service() {
    let mut mock_service = MockCliService::default();
    for arguments in [
        vec!["export", "meeting-rome", "--format", "md", "--no-metadata"],
        vec!["export-all", "--format", "pdf", "--output", "/tmp/all.pdf"],
    ] {
        let command = parse_export_command(&arguments)
            .unwrap_or_else(|error| panic!("export command must parse: {error}"));
        dispatch(
            CliCommand::Meetings(MeetingArgs { command }),
            &mut mock_service,
        )
        .unwrap_or_else(|error| panic!("export command must dispatch: {error}"));
    }

    let export_requests = mock_service.export_requests.into_inner();
    assert_eq!(
        export_requests[0],
        (
            Some(String::from("meeting-rome")),
            ExportOptions::new(ExportFormat::Markdown, true, false, None),
            None
        )
    );
    assert_eq!(
        export_requests[1],
        (
            None,
            ExportOptions::for_format(ExportFormat::Pdf),
            Some(PathBuf::from("/tmp/all.pdf"))
        )
    );
}

/// Verifies the concrete CLI service exports an in-memory SQLite meeting through ExportManager.
#[test]
fn core_service_exports_meeting_markdown_to_requested_output() {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-cli-export");
    repository
        .create_meeting(&meeting_id, &[1])
        .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
    repository
        .save_transcript_segments(
            &meeting_id,
            &[TranscriptSegment::new(
                meeting_id.clone(),
                0,
                2_000,
                None,
                "Exported from the CLI.",
            )],
        )
        .unwrap_or_else(|error| panic!("fixture segments must persist: {error}"));
    let export_directory =
        std::env::temp_dir().join(format!("note-taker-cli-export-{}", Uuid::new_v4()));
    let output_path = export_directory.join("minutes.md");
    let mut cli_service = CoreCliService::new(
        NoRecordingControl,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), &export_directory),
    );
    let command = parse_export_command(&[
        "export",
        "meeting-cli-export",
        "--format",
        "markdown",
        "--output",
        &output_path.to_string_lossy(),
    ])
    .unwrap_or_else(|error| panic!("export command must parse: {error}"));

    let output = dispatch(
        CliCommand::Meetings(MeetingArgs { command }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("export must succeed: {error}"));
    let written_markdown = std::fs::read_to_string(&output_path)
        .unwrap_or_else(|error| panic!("export must be written: {error}"));
    let _ = std::fs::remove_dir_all(&export_directory);

    assert!(output.starts_with("Exported 1 meeting as markdown to "));
    assert!(output.contains("minutes.md"));
    assert!(written_markdown.contains("Exported from the CLI."));
}

/// Verifies an unknown meeting surfaces a readable error through the CLI service.
#[test]
fn core_service_reports_unknown_meeting_export() {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    let cli_service = CoreCliService::new(
        NoRecordingControl,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), std::env::temp_dir()),
    );

    let export_failure = cli_service
        .export_meeting(
            "missing-meeting",
            &ExportOptions::for_format(ExportFormat::Json),
            None,
        )
        .expect_err("unknown meetings must fail");

    assert!(export_failure.contains("meeting missing-meeting does not exist"));
}

/// Verifies `meetings rename` parses the meeting ID and a title that contains spaces, and
/// requires both.
#[test]
fn parses_meetings_rename_command() {
    let parsed_command = parse_export_command(&["rename", "meeting-rome", "Rome offsite"])
        .unwrap_or_else(|error| panic!("rename must parse: {error}"));
    assert!(
        matches!(
            &parsed_command,
            MeetingCommand::Rename { meeting_id, new_title }
                if meeting_id == "meeting-rome" && new_title == "Rome offsite"
        ),
        "{parsed_command:?}"
    );

    assert!(parse_export_command(&["rename", "meeting-rome"]).is_err());
    assert!(parse_export_command(&["rename"]).is_err());
}

/// Verifies `meetings rename` delegates its arguments to the service exactly once.
#[test]
fn meetings_rename_delegates_to_the_service() {
    let mut cli_service = MockCliService::default();

    let output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::Rename {
                meeting_id: String::from("meeting-rome"),
                new_title: String::from("Rome offsite"),
            },
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("rename must dispatch: {error}"));

    assert_eq!(output, "meeting renamed");
    assert_eq!(
        cli_service.rename_requests,
        vec![(String::from("meeting-rome"), String::from("Rome offsite"))]
    );
}

/// Builds a CLI service over in-memory SQLite holding one meeting titled `Project Sync`.
fn core_service_with_titled_meeting()
-> CoreCliService<NoRecordingControl, SqliteNoteRepository, InMemoryVoiceVectorDB> {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    crate::application::meeting_crdt::MeetingCrdtService::new(&repository)
        .create_titled_meeting(
            &MeetingId::new("meeting-rename"),
            &MeetingTitle::parse("Project Sync")
                .unwrap_or_else(|error| panic!("title must parse: {error}")),
        )
        .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
    CoreCliService::new(
        NoRecordingControl,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), std::env::temp_dir()),
    )
}

/// Verifies `meetings rename` stores the trimmed title, prints the old and new titles, and
/// `meetings list` then shows the new title.
#[test]
fn core_service_renames_a_meeting_and_prints_both_titles() {
    let mut cli_service = core_service_with_titled_meeting();

    let rename_output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::Rename {
                meeting_id: String::from("meeting-rename"),
                new_title: String::from("  Project Sync (Q4)  "),
            },
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("rename must succeed: {error}"));
    let list_output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::List,
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("meeting list must dispatch: {error}"));

    assert_eq!(
        rename_output,
        r#"Renamed meeting meeting-rename: "Project Sync" -> "Project Sync (Q4)""#
    );
    assert!(
        list_output.contains("meeting-rename | Project Sync (Q4) |"),
        "{list_output}"
    );
}

/// Verifies unknown meetings and invalid titles produce readable errors and keep the title.
#[test]
fn core_service_rejects_unknown_meetings_and_invalid_titles_on_rename() {
    let mut cli_service = core_service_with_titled_meeting();

    let missing_error = cli_service
        .rename_meeting("meeting-missing", "Orphan")
        .expect_err("an unknown meeting must not be renamed");
    let blank_error = cli_service
        .rename_meeting("meeting-rename", "   ")
        .expect_err("a blank title must be rejected");
    let multi_line_error = cli_service
        .rename_meeting("meeting-rename", "line one\nline two")
        .expect_err("a multi-line title must be rejected");
    let oversized_error = cli_service
        .rename_meeting("meeting-rename", &"x".repeat(201))
        .expect_err("an oversized title must be rejected");
    let list_output = cli_service
        .list_meetings()
        .unwrap_or_else(|error| panic!("meetings must list: {error}"));

    assert_eq!(missing_error, "meeting meeting-missing does not exist");
    assert_eq!(
        blank_error,
        "invalid title: meeting title must not be blank"
    );
    assert_eq!(
        multi_line_error,
        "invalid title: meeting title must not contain control characters"
    );
    assert_eq!(
        oversized_error,
        "invalid title: meeting title must be at most 200 characters"
    );
    assert_eq!(list_output[0].title, "Project Sync");
}

/// Verifies `voice list-clusters` and `voice assign-cluster` parse their arguments.
#[test]
fn parses_cluster_review_commands() {
    let listed = Cli::try_parse_from(["note-taker", "voice", "list-clusters", "meeting-rome"])
        .unwrap_or_else(|error| panic!("voice list-clusters must parse: {error}"));
    assert!(
        matches!(listed.command, CliCommand::Voice(VoiceArgs { command: VoiceCommand::ListClusters { meeting_id } }) if meeting_id == "meeting-rome")
    );

    let cluster_id = Uuid::new_v4();
    let assigned = Cli::try_parse_from([
        "note-taker",
        "voice",
        "assign-cluster",
        &cluster_id.to_string(),
        "Grace Hopper",
    ])
    .unwrap_or_else(|error| panic!("voice assign-cluster must parse: {error}"));
    assert!(
        matches!(assigned.command, CliCommand::Voice(VoiceArgs { command: VoiceCommand::AssignCluster { cluster_id: parsed_cluster_id, identity_name } }) if parsed_cluster_id == cluster_id && identity_name == "Grace Hopper")
    );
}

/// Verifies malformed cluster IDs and missing names are rejected by clap.
#[test]
fn rejects_malformed_cluster_assignment_arguments() {
    assert!(
        Cli::try_parse_from([
            "note-taker",
            "voice",
            "assign-cluster",
            "not-a-uuid",
            "Grace"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "note-taker",
            "voice",
            "assign-cluster",
            &Uuid::new_v4().to_string()
        ])
        .is_err()
    );
}

/// Verifies `list-clusters` formats the service rows as a table.
#[test]
fn list_clusters_formats_mocked_rows_as_a_table() {
    let mut mock_service = MockCliService {
        cluster_rows: vec![
            (String::from("cluster-a"), 3_500, 2),
            (String::from("cluster-b"), 65_250, 7),
        ],
        ..MockCliService::default()
    };

    let output = dispatch(
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::ListClusters {
                meeting_id: String::from("meeting-rome"),
            },
        }),
        &mut mock_service,
    )
    .unwrap_or_else(|error| panic!("list-clusters must dispatch: {error}"));

    assert_eq!(
        output,
        "Cluster ID | Total Duration | Chunks\ncluster-a | 3.5s | 2\ncluster-b | 65.3s | 7"
    );
    assert_eq!(
        mock_service.listed_cluster_meetings.into_inner(),
        vec![String::from("meeting-rome")]
    );
}

/// Verifies `list-clusters` explains an empty result instead of printing a bare header.
#[test]
fn list_clusters_reports_when_no_cluster_is_unresolved() {
    let mut mock_service = MockCliService::default();

    let output = dispatch(
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::ListClusters {
                meeting_id: String::from("meeting-rome"),
            },
        }),
        &mut mock_service,
    )
    .unwrap_or_else(|error| panic!("list-clusters must dispatch: {error}"));

    assert_eq!(
        output,
        "No unresolved unknown-speaker clusters for meeting meeting-rome"
    );
}

/// Verifies `assign-cluster` passes the parsed cluster ID and typed name to the service once.
#[test]
fn assign_cluster_delegates_arguments_to_the_service() {
    let cluster_id = Uuid::new_v4();
    let mut mock_service = MockCliService::default();

    let output = dispatch(
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::AssignCluster {
                cluster_id,
                identity_name: String::from("Grace Hopper"),
            },
        }),
        &mut mock_service,
    )
    .unwrap_or_else(|error| panic!("assign-cluster must dispatch: {error}"));

    assert_eq!(output, "cluster assigned");
    assert_eq!(
        mock_service.assigned_clusters.into_inner(),
        vec![(cluster_id, String::from("Grace Hopper"))]
    );
}

/// Builds the concrete CLI service over in-memory SQLite holding one meeting with one cluster.
///
/// Outputs: The service and the cluster ID (two chunks, 2.5 s total).
fn core_service_with_cluster() -> (
    CoreCliService<NoRecordingControl, SqliteNoteRepository, InMemoryVoiceVectorDB>,
    Uuid,
) {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-clusters");
    repository
        .create_meeting(&meeting_id, &[1])
        .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
    let clusterer = UnknownSpeakerClusterer::new(&repository, ClusteringConfig::default());
    let cluster_id = clusterer
        .find_or_create_cluster(&meeting_id, &[1.0, 0.0], String::from("c1"), 1_000)
        .unwrap_or_else(|error| panic!("fixture chunk must cluster: {error}"))
        .cluster_id();
    clusterer
        .find_or_create_cluster(&meeting_id, &[0.9, 0.3], String::from("c2"), 1_500)
        .unwrap_or_else(|error| panic!("fixture chunk must cluster: {error}"));
    (
        CoreCliService::new(
            NoRecordingControl,
            repository,
            InMemoryVoiceVectorDB::new(),
            ExportWriter::new(default_exporters(), std::env::temp_dir()),
        ),
        cluster_id,
    )
}

/// Verifies the concrete service lists and assigns clusters through the cluster-review use case.
#[test]
fn core_service_lists_and_assigns_clusters_from_sqlite() {
    let (mut cli_service, cluster_id) = core_service_with_cluster();
    let list_command = || {
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::ListClusters {
                meeting_id: String::from("meeting-clusters"),
            },
        })
    };

    let listed = dispatch(list_command(), &mut cli_service)
        .unwrap_or_else(|error| panic!("list-clusters must dispatch: {error}"));
    assert!(listed.contains(&format!("{cluster_id} | 2.5s | 2")));

    let assigned = dispatch(
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::AssignCluster {
                cluster_id,
                identity_name: String::from("Grace Hopper"),
            },
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("assign-cluster must dispatch: {error}"));
    assert!(assigned.starts_with(&format!(
        "Assigned cluster {cluster_id} to \"Grace Hopper\""
    )));
    assert!(assigned.contains("new identity"));
    assert!(assigned.contains("0 voice variants stored, 0 transcript segments updated"));

    let relisted = dispatch(list_command(), &mut cli_service)
        .unwrap_or_else(|error| panic!("list-clusters must dispatch: {error}"));
    assert!(relisted.starts_with("No unresolved unknown-speaker clusters"));
}

/// Verifies the concrete service reports blank names and unknown clusters as errors.
#[test]
fn core_service_rejects_blank_names_and_unknown_clusters() {
    let (mut cli_service, cluster_id) = core_service_with_cluster();

    let blank = cli_service.assign_cluster(cluster_id, "  ");
    assert_eq!(blank, Err(String::from("identity name must not be blank")));

    let missing_cluster_id = Uuid::new_v4();
    let missing = cli_service.assign_cluster(missing_cluster_id, "Grace");
    assert_eq!(
        missing,
        Err(format!(
            "unknown-speaker cluster {missing_cluster_id} was not found"
        ))
    );
}

/// Opens the concrete CLI service over the SQLite file at `database_path`, as `bin/cli.rs` does.
fn file_backed_service(
    database_path: &Path,
) -> CoreCliService<NoRecordingControl, SqliteNoteRepository, SqliteVoiceVectorDB> {
    let crate::storage_composition::EncryptedStores {
        note_repository: repository,
        voice_store,
        ..
    } = crate::storage_composition::open_encrypted_stores(
        database_path,
        DeviceId::new("cli-test"),
        crate::storage_composition::key_file_config_for_tests(
            &database_path.with_extension("keys"),
        ),
    )
    .unwrap_or_else(|error| panic!("encrypted stores must open: {error:#}"));
    CoreCliService::new(
        NoRecordingControl,
        repository,
        voice_store,
        ExportWriter::new(default_exporters(), std::env::temp_dir()),
    )
}

/// Verifies `voice assign-cluster` names survive a restart: a new CLI process lists them.
#[test]
fn assigned_cluster_names_are_listed_after_a_restart() {
    let database_path =
        std::env::temp_dir().join(format!("note-taker-cli-voice-{}.sqlite3", Uuid::new_v4()));
    let meeting_id = MeetingId::new("meeting-cli-persist");
    let assigned = {
        let mut first_run = file_backed_service(&database_path);
        let clusterer =
            UnknownSpeakerClusterer::new(&first_run.repository, ClusteringConfig::default());
        first_run
            .repository
            .create_meeting(&meeting_id, &[])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
        let cluster_id = clusterer
            .find_or_create_cluster(&meeting_id, &[1.0, 0.0], String::from("c1"), 1_000)
            .unwrap_or_else(|error| panic!("fixture chunk must cluster: {error}"))
            .cluster_id();
        first_run.assign_cluster(cluster_id, "Grace Hopper")
    };

    let mut second_run = file_backed_service(&database_path);
    let listed = dispatch(
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::List,
        }),
        &mut second_run,
    );
    drop(second_run);
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", database_path.display()));
    }
    let _ = std::fs::remove_dir_all(database_path.with_extension("keys"));

    assert!(assigned.is_ok_and(|output| output.contains("new identity")));
    let listed = listed.unwrap_or_else(|error| panic!("voice list must dispatch: {error}"));
    assert!(listed.contains("Grace Hopper"), "{listed}");
    assert!(listed.contains("| 0"), "{listed}");
}

/// Verifies the global `--log-level` flag parses before or after the subcommand and is optional.
#[test]
fn parses_the_global_log_level_flag() {
    let before = Cli::try_parse_from(["note-taker", "--log-level", "debug", "meetings", "list"])
        .unwrap_or_else(|error| panic!("a leading --log-level must parse: {error}"));
    let after = Cli::try_parse_from(["note-taker", "meetings", "list", "--log-level", "trace"])
        .unwrap_or_else(|error| panic!("a trailing --log-level must parse: {error}"));
    let absent = Cli::try_parse_from(["note-taker", "meetings", "list"])
        .unwrap_or_else(|error| panic!("the flag must be optional: {error}"));

    assert_eq!(before.log_level.as_deref(), Some("debug"));
    assert_eq!(after.log_level.as_deref(), Some("trace"));
    assert_eq!(absent.log_level, None);
}

/// Verifies `log-level <directive>` is dispatched to the service.
#[test]
fn log_level_command_requests_the_daemon_level() {
    let parsed_cli = Cli::try_parse_from(["note-taker", "log-level", "note_taker_core=debug"])
        .unwrap_or_else(|error| panic!("log-level must parse: {error}"));
    let mut cli_service = MockCliService::default();

    let output = dispatch(parsed_cli.command, &mut cli_service);

    assert_eq!(output, Ok(String::from("log level requested")));
    assert_eq!(cli_service.log_level_requests, ["note_taker_core=debug"]);
    assert!(Cli::try_parse_from(["note-taker", "log-level"]).is_err());
}

/// Builds a core service over in-memory storage.
fn in_memory_core_service()
-> CoreCliService<NoRecordingControl, SqliteNoteRepository, InMemoryVoiceVectorDB> {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    CoreCliService::new(
        NoRecordingControl,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), std::env::temp_dir()),
    )
}

/// Verifies the core service writes a validated level to the daemon's control file.
#[test]
fn core_service_writes_the_requested_level_for_the_daemon() {
    let directory = std::env::temp_dir().join(format!("note-taker-cli-log-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
    let control_file = LogLevelControlFile::for_database(&directory.join("note_taker.sqlite3"));
    let mut cli_service = in_memory_core_service().with_log_level_control(control_file.clone());

    let output = cli_service.request_log_level("debug");
    let rejected = cli_service.request_log_level("note_taker_core=loud");

    assert!(output.is_ok_and(|message| message.contains("debug")));
    assert!(rejected.is_err());
    assert_eq!(control_file.read().ok(), Some(Some("debug".to_owned())));
    let _ = std::fs::remove_dir_all(directory);
}

/// Verifies a service composed without a control file reports that instead of panicking.
#[test]
fn core_service_without_a_control_file_rejects_log_level_requests() {
    assert!(in_memory_core_service().request_log_level("debug").is_err());
}

/// Without `--log-level`, the log file keeps the production default `info` (REQUIREMENTS 6.1)
/// while stdout shows only errors, so JSON logs do not mix with command output.
#[test]
fn default_log_filters_keep_info_in_the_file_and_errors_on_stdout() {
    let parsed_cli = Cli::parse_from(["note-taker", "meetings", "list"]);

    let log_filters = parsed_cli.log_filters();

    assert_eq!(log_filters.file_filter, "info");
    assert_eq!(log_filters.stdout_filter, "error");
}

/// An explicit `--log-level` applies to both sinks, so `--log-level debug` shows debug output.
#[test]
fn explicit_log_level_applies_to_file_and_stdout() {
    let parsed_cli = Cli::parse_from(["note-taker", "--log-level", "debug", "meetings", "list"]);

    let log_filters = parsed_cli.log_filters();

    assert_eq!(log_filters.file_filter, "debug");
    assert_eq!(log_filters.stdout_filter, "debug");
}

/// Verifies `devices list` prints one row per paired device, or a sentence when there are none.
#[test]
fn devices_list_prints_the_paired_devices() {
    let parse = || {
        Cli::try_parse_from(["note-taker", "devices", "list"])
            .unwrap_or_else(|error| panic!("devices list must parse: {error}"))
            .command
    };
    let mut cli_service = MockCliService {
        paired_device_rows: vec![PairedDeviceRow {
            peer_id: String::from("12D3KooWPhone"),
            device_name: String::from("Phone"),
            paired_at: String::from("2026-10-28 12:00 UTC"),
        }],
        ..MockCliService::default()
    };

    let listed = dispatch(parse(), &mut cli_service);
    let empty = dispatch(parse(), &mut MockCliService::default());

    assert_eq!(
        listed,
        Ok(String::from(
            "Peer ID | Name | Paired\n12D3KooWPhone | Phone | 2026-10-28 12:00 UTC"
        ))
    );
    assert_eq!(empty, Ok(String::from("No paired devices.")));
}

/// Verifies `devices revoke <peer ID or name>` is dispatched to the service.
#[test]
fn devices_revoke_is_dispatched_with_its_selector() {
    let parsed_cli = Cli::try_parse_from(["note-taker", "devices", "revoke", "Phone"])
        .unwrap_or_else(|error| panic!("devices revoke must parse: {error}"));
    let mut cli_service = MockCliService::default();

    let output = dispatch(parsed_cli.command, &mut cli_service);

    assert_eq!(output, Ok(String::from("device removed")));
    assert_eq!(cli_service.revoked_devices, ["Phone"]);
    assert!(Cli::try_parse_from(["note-taker", "devices", "revoke"]).is_err());
}

/// Builds a core service whose allowlist holds the given `(peer ID, name, paired at ms)` peers.
fn core_service_with_paired_devices(
    peers: &[(&str, &str, u64)],
) -> CoreCliService<NoRecordingControl, SqliteNoteRepository, InMemoryVoiceVectorDB> {
    let trust_store = SqlitePeerStore::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("peer store must open: {error}"));
    for (peer_id, device_name, paired_at_ms) in peers {
        let device_name = DeviceName::parse(device_name)
            .unwrap_or_else(|error| panic!("valid device name: {error}"));
        trust_store
            .trust_peer(&TrustedPeer::new(
                DeviceId::new(*peer_id),
                device_name,
                *paired_at_ms,
            ))
            .unwrap_or_else(|error| panic!("peer must be stored: {error}"));
    }
    in_memory_core_service().with_trusted_peer_store(Box::new(trust_store))
}

/// Verifies the core service lists the allowlist with UTC pairing times.
#[test]
fn core_service_lists_paired_devices() {
    let cli_service = core_service_with_paired_devices(&[
        ("12D3KooWPhone", "Phone", 1_793_188_800_000),
        ("12D3KooWLaptop", "Laptop", 1_793_192_400_000),
    ]);

    let rows = cli_service.list_paired_devices();

    assert_eq!(
        rows,
        Ok(vec![
            PairedDeviceRow {
                peer_id: String::from("12D3KooWPhone"),
                device_name: String::from("Phone"),
                paired_at: String::from("2026-10-28 12:00 UTC"),
            },
            PairedDeviceRow {
                peer_id: String::from("12D3KooWLaptop"),
                device_name: String::from("Laptop"),
                paired_at: String::from("2026-10-28 13:00 UTC"),
            },
        ])
    );
}

/// Verifies a device is removed by peer ID or by its unique name (any case), and that an unknown
/// selector changes nothing.
#[test]
fn core_service_revokes_by_peer_id_or_unique_name() {
    let mut cli_service = core_service_with_paired_devices(&[
        ("12D3KooWPhone", "Phone", 1),
        ("12D3KooWLaptop", "Laptop", 2),
    ]);

    let by_name = cli_service.revoke_device("phone");
    let by_peer_id = cli_service.revoke_device("12D3KooWLaptop");
    let unknown = cli_service.revoke_device("Tablet");

    assert!(by_name.is_ok_and(|message| message.starts_with("Removed Phone (12D3KooWPhone).")));
    assert!(
        by_peer_id.is_ok_and(|message| message.starts_with("Removed Laptop (12D3KooWLaptop)."))
    );
    assert_eq!(
        unknown,
        Err(String::from(
            "No paired device has the peer ID or name \"Tablet\"."
        ))
    );
    assert_eq!(cli_service.list_paired_devices(), Ok(Vec::new()));
}

/// Verifies a name shared by several devices is refused, so the wrong device is never removed.
#[test]
fn core_service_refuses_an_ambiguous_name() {
    let mut cli_service = core_service_with_paired_devices(&[
        ("12D3KooWPhoneA", "Phone", 1),
        ("12D3KooWPhoneB", "Phone", 2),
    ]);

    let outcome = cli_service.revoke_device("Phone");

    assert_eq!(
        outcome,
        Err(String::from(
            "2 paired devices are named \"Phone\"; use the peer ID from `devices list`."
        ))
    );
    assert_eq!(
        cli_service.list_paired_devices().map(|rows| rows.len()),
        Ok(2)
    );
}

/// Verifies a service composed without the allowlist reports that instead of panicking.
#[test]
fn core_service_without_an_allowlist_rejects_device_commands() {
    let mut cli_service = in_memory_core_service();

    assert!(cli_service.list_paired_devices().is_err());
    assert!(cli_service.revoke_device("Phone").is_err());
}

/// Verifies the delete commands parse and need --yes before anything goes.
#[test]
fn meeting_delete_needs_yes_and_then_removes_it() {
    let mut service = core_service_with_titled_meeting();
    let parse = |arguments: &[&str]| {
        Cli::try_parse_from(arguments).unwrap_or_else(|error| panic!("must parse: {error}"))
    };

    let refused = dispatch(
        parse(&["note-taker", "meetings", "delete", "meeting-rename"]).command,
        &mut service,
    );
    assert!(refused.is_err_and(|message| message.contains("--yes")));
    assert!(service.list_meetings().is_ok_and(|rows| rows.len() == 1));

    let deleted = dispatch(
        parse(&[
            "note-taker",
            "meetings",
            "delete",
            "meeting-rename",
            "--yes",
        ])
        .command,
        &mut service,
    );
    assert_eq!(deleted.as_deref(), Ok("Deleted meeting meeting-rename"));
    assert!(service.list_meetings().is_ok_and(|rows| rows.is_empty()));

    let repeated = dispatch(
        parse(&[
            "note-taker",
            "meetings",
            "delete",
            "meeting-rename",
            "--yes",
        ])
        .command,
        &mut service,
    );
    assert_eq!(
        repeated,
        Err(String::from("meeting meeting-rename does not exist"))
    );
}

/// Verifies the voice delete commands parse and an unknown voice is reported.
#[test]
fn voice_delete_commands_report_their_outcome() {
    let mut service = core_service_with_titled_meeting();
    let unknown = Uuid::from_u128(7).to_string();
    let parse = |arguments: &[&str]| {
        Cli::try_parse_from(arguments).unwrap_or_else(|error| panic!("must parse: {error}"))
    };

    let unconfirmed = dispatch(
        parse(&["note-taker", "voice", "delete-person", &unknown]).command,
        &mut service,
    );
    let cluster = dispatch(
        parse(&["note-taker", "voice", "delete-cluster", &unknown, "--yes"]).command,
        &mut service,
    );
    let person = dispatch(
        parse(&["note-taker", "voice", "delete-person", &unknown, "--yes"]).command,
        &mut service,
    );

    assert!(unconfirmed.is_err_and(|message| message.contains("--yes")));
    assert_eq!(
        cluster,
        Err(format!("unknown voice {unknown} does not exist"))
    );
    assert_eq!(person, Ok(format!("Deleted person {unknown}")));
}

/// Verifies a meeting recorded on two devices is listed once with both devices, its view says so,
/// and `meetings split` lists the other device's recording again (#88).
#[test]
fn core_service_lists_a_combined_meeting_once_and_splits_it() {
    use crate::{
        application::meeting_document::AutomergeMeetingDocumentCodec,
        ports::meeting_document::MeetingCombination,
    };
    let directory = std::env::temp_dir().join(format!("cli-combined-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap_or_else(|error| panic!("{error}"));
    let cipher = crate::application::crypto::voice_encryptor::ephemeral_cipher();
    let open = |device: &str| {
        SqliteNoteRepository::open(
            directory.join("notes.sqlite3"),
            crate::domain::models::DeviceId::new(device),
            std::sync::Arc::clone(&cipher),
        )
        .and_then(|repository| {
            repository.with_document_codec(std::sync::Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"))
    };
    let laptop = open("device-laptop");
    let phone = open("device-phone");
    let empty_document = automerge::AutoCommit::new().save();
    laptop
        .create_meeting(&MeetingId::new("meeting-laptop"), &empty_document)
        .unwrap_or_else(|error| panic!("{error}"));
    phone
        .create_meeting(&MeetingId::new("meeting-phone"), &empty_document)
        .unwrap_or_else(|error| panic!("{error}"));
    laptop
        .combine_meeting(
            &MeetingId::new("meeting-phone"),
            &MeetingCombination {
                into: MeetingId::new("meeting-laptop"),
                offset_ms: 2_000,
                combine_on: crate::domain::models::DeviceId::new("device-laptop"),
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));
    drop(phone);
    let mut cli_service = CoreCliService::new(
        NoRecordingControl,
        laptop,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), std::env::temp_dir()),
    );
    let list = |cli_service: &mut CoreCliService<_, _, _>| {
        dispatch(
            CliCommand::Meetings(MeetingArgs {
                command: MeetingCommand::List,
            }),
            cli_service,
        )
        .unwrap_or_else(|error| panic!("meetings must list: {error}"))
    };

    let combined = list(&mut cli_service);
    let view = cli_service
        .view_meeting("meeting-laptop")
        .unwrap_or_else(|error| panic!("{error}"));
    let split = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::Split {
                meeting_id: String::from("meeting-laptop"),
            },
        }),
        &mut cli_service,
    );
    let apart = list(&mut cli_service);
    let _ = std::fs::remove_dir_all(&directory);

    assert!(
        combined.starts_with("ID | Title | Date | Duration | Devices"),
        "{combined}"
    );
    assert!(
        combined.contains("meeting-laptop |") && combined.ends_with("| 2"),
        "{combined}"
    );
    assert!(!combined.contains("meeting-phone"), "{combined}");
    assert!(
        view.contains("Recorded on 2 devices (also meeting-phone)"),
        "{view}"
    );
    assert!(
        view.contains("Combining: waiting for speech from device-phone to reach device-laptop\n"),
        "{view}"
    );
    assert_eq!(
        split,
        Ok(String::from(
            "Split meeting meeting-laptop: meeting-phone is listed on its own again"
        ))
    );
    assert!(apart.contains("meeting-phone |"), "{apart}");
}

/// Runs one CLI invocation against `cli_service`.
fn run(arguments: &[&str], cli_service: &mut MockCliService) -> Result<String, String> {
    let parsed_cli = Cli::try_parse_from(arguments.iter().copied())
        .unwrap_or_else(|error| panic!("{arguments:?} must parse: {error}"));
    dispatch(parsed_cli.command, cli_service)
}

/// Verifies the glossary can be set, extended, trimmed, shown and cleared from the CLI.
#[test]
fn the_glossary_is_edited_term_by_term() {
    let mut cli_service = MockCliService::default();

    run(
        &["note-taker", "glossary", "set", "Marco Rossi", "OKR"],
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    run(
        &["note-taker", "glossary", "add", "Kubernetes", "okr"],
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(cli_service.glossary, ["Marco Rossi", "OKR", "Kubernetes"]);
    run(
        &["note-taker", "glossary", "remove", "OKR"],
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let shown = run(&["note-taker", "glossary", "show"], &mut cli_service)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(shown, "Marco Rossi\nKubernetes");
    run(&["note-taker", "glossary", "clear"], &mut cli_service)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(cli_service.glossary.is_empty());
    assert_eq!(
        run(&["note-taker", "glossary", "show"], &mut cli_service),
        Ok(String::from("The glossary is empty."))
    );
}

/// Verifies an invalid glossary is refused with the reason and nothing is saved.
#[test]
fn an_invalid_glossary_is_refused() {
    let mut cli_service = MockCliService::default();
    let long_term = "x".repeat(61);

    let result = run(
        &["note-taker", "glossary", "set", &long_term],
        &mut cli_service,
    );

    assert!(result.is_err_and(|error| error.contains("60 characters")));
    assert!(cli_service.glossary.is_empty());
}
