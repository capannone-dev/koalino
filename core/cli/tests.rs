//! Defines parser and service-dispatch contracts before the CLI adapter implementation.

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use uuid::Uuid;

use super::{
    Cli, CliCommand, CliService, CoreCliService, MeetingArgs, MeetingCommand, RecordArgs,
    RecordCommand, VoiceArgs, VoiceCommand, dispatch,
};
use crate::{
    adapters::{
        audio::local_recorder::LocalRecorder,
        export::default_exporters,
        storage::{
            in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
            sqlite_voice_vector_db::SqliteVoiceVectorDB,
        },
    },
    application::export_manager::ExportWriter,
    application::unknown_speaker_clusterer::{ClusteringConfig, UnknownSpeakerClusterer},
    domain::models::{
        AudioConfig, DeviceId, ExportFormat, ExportOptions, MeetingId, TranscriptSegment,
    },
    ports::storage::NoteRepository,
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
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-in-memory");
    repository
        .create_meeting(&meeting_id, &[1, 2, 3])
        .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
    let recorder = LocalRecorder::new(AudioConfig::new(16_000, 1))
        .unwrap_or_else(|error| panic!("fixture recorder must initialize: {error}"));
    let mut cli_service = CoreCliService::new(
        recorder,
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
    let recorder = LocalRecorder::new(AudioConfig::new(16_000, 1))
        .unwrap_or_else(|error| panic!("fixture recorder must initialize: {error}"));
    let export_directory =
        std::env::temp_dir().join(format!("note-taker-cli-export-{}", Uuid::new_v4()));
    let output_path = export_directory.join("minutes.md");
    let mut cli_service = CoreCliService::new(
        recorder,
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
    let recorder = LocalRecorder::new(AudioConfig::new(16_000, 1))
        .unwrap_or_else(|error| panic!("fixture recorder must initialize: {error}"));
    let cli_service = CoreCliService::new(
        recorder,
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
    CoreCliService<LocalRecorder, SqliteNoteRepository, InMemoryVoiceVectorDB>,
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
    let recorder = LocalRecorder::new(AudioConfig::new(16_000, 1))
        .unwrap_or_else(|error| panic!("fixture recorder must initialize: {error}"));
    (
        CoreCliService::new(
            recorder,
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
    assert!(assigned.contains("2 voice variants stored, 0 transcript segments updated"));

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
) -> CoreCliService<LocalRecorder, SqliteNoteRepository, SqliteVoiceVectorDB> {
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
    let recorder = LocalRecorder::new(AudioConfig::new(16_000, 1))
        .unwrap_or_else(|error| panic!("fixture recorder must initialize: {error}"));
    CoreCliService::new(
        recorder,
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
            .create_meeting(&meeting_id, &[1])
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
    assert!(listed.contains("| 1"), "{listed}");
}
