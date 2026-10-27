//! Defines the CLI driving-adapter boundary and its test-first command contract.
//!
//! Command parsing and terminal formatting stay here while domain policy and persistence remain
//! behind existing application services and repository ports.

use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveTime};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

use crate::{
    application::{
        cluster_review::{
            NamedClusterAssignment, assign_cluster_to_named_identity, list_unknown_clusters,
        },
        export_manager::{ExportManager, ExportWriter},
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::models::{ExportDateRange, ExportFormat, ExportOptions, ExportResult, MeetingId},
    ports::{
        audio::AudioCapture,
        storage::{AttachmentRepository, NoteRepository},
        telemetry::MetricsRepository,
        voice::VoiceVectorDB,
    },
};

/// Parses terminal arguments without accessing adapters or application state.
#[derive(Debug, Parser)]
#[command(
    name = "note-taker",
    about = "Manage offline-first meeting notes locally"
)]
pub struct Cli {
    /// Selects the requested capability.
    #[command(subcommand)]
    pub command: CliCommand,
}

/// Defines top-level command groups.
#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Controls portable audio recording.
    Record(RecordArgs),
    /// Reads locally persisted meeting data.
    Meetings(MeetingArgs),
    /// Lists or resolves voice review items.
    Voice(VoiceArgs),
    /// Displays aggregate privacy-safe telemetry.
    Telemetry,
}

/// Defines recording state actions.
#[derive(Debug, Subcommand)]
pub enum RecordCommand {
    Start,
    Stop,
    Status,
}

/// Holds a nested recording action for clap.
#[derive(Debug, Args)]
pub struct RecordArgs {
    /// Selects the recording action.
    #[command(subcommand)]
    pub command: RecordCommand,
}

/// Defines meeting read and export actions.
#[derive(Debug, Subcommand)]
pub enum MeetingCommand {
    List,
    View {
        meeting_id: String,
    },
    /// Exports one meeting as Markdown, PDF, or JSON.
    Export {
        /// The meeting to export.
        meeting_id: String,
        /// Shared format, destination, and inclusion flags.
        #[command(flatten)]
        export_arguments: ExportCommandArgs,
    },
    /// Exports every meeting (optionally within a creation date range) into one file.
    ExportAll {
        /// Inclusive UTC day range `YYYY-MM-DD:YYYY-MM-DD` filtering meetings by creation date.
        #[arg(long, value_parser = parse_export_date_range)]
        date_range: Option<ExportDateRange>,
        /// Shared format, destination, and inclusion flags.
        #[command(flatten)]
        export_arguments: ExportCommandArgs,
    },
}

/// Holds the flags shared by `meetings export` and `meetings export-all`.
#[derive(Debug, Args)]
pub struct ExportCommandArgs {
    /// Output format: markdown (md), pdf, or json.
    #[arg(long, value_parser = parse_export_format)]
    pub format: ExportFormat,
    /// Destination file; defaults to a generated name in the export directory.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Omits attachments from the export.
    #[arg(long)]
    pub no_attachments: bool,
    /// Omits the meeting metadata header (date, duration, participants).
    #[arg(long)]
    pub no_metadata: bool,
}

impl ExportCommandArgs {
    /// Converts parsed flags into portable export options.
    ///
    /// Inputs: The optional batch `date_range`.
    /// Outputs: `ExportOptions` with attachments and metadata included unless disabled.
    /// Side effects: None.
    pub fn export_options(&self, date_range: Option<ExportDateRange>) -> ExportOptions {
        ExportOptions::new(
            self.format,
            !self.no_attachments,
            !self.no_metadata,
            date_range,
        )
    }
}

/// Parses a case-insensitive export format name for clap.
///
/// Inputs: Raw `--format` text.
/// Outputs: The format or a user-readable error.
/// Side effects: None.
fn parse_export_format(raw_format: &str) -> Result<ExportFormat, String> {
    raw_format
        .parse::<ExportFormat>()
        .map_err(|error| error.to_string())
}

/// Parses `YYYY-MM-DD:YYYY-MM-DD` into an inclusive range covering both whole UTC days.
///
/// Inputs: Raw `--date-range` text.
/// Outputs: A range from the start day's midnight to the end day's last millisecond, or an
/// error for malformed dates or a start after the end.
/// Side effects: None.
fn parse_export_date_range(raw_range: &str) -> Result<ExportDateRange, String> {
    let (raw_start, raw_end) = raw_range
        .split_once(':')
        .ok_or_else(|| String::from("expected YYYY-MM-DD:YYYY-MM-DD"))?;
    let parse_day = |raw_day: &str| {
        NaiveDate::parse_from_str(raw_day.trim(), "%Y-%m-%d")
            .map_err(|error| format!("invalid date `{raw_day}`: {error}"))
    };
    let last_millisecond = NaiveTime::from_hms_milli_opt(23, 59, 59, 999)
        .ok_or_else(|| String::from("unable to build end-of-day time"))?;
    ExportDateRange::new(
        parse_day(raw_start)?.and_time(NaiveTime::MIN).and_utc(),
        parse_day(raw_end)?.and_time(last_millisecond).and_utc(),
    )
    .map_err(|error| error.to_string())
}

/// Formats a successful export for terminal output.
fn format_export_result(export_result: &ExportResult) -> String {
    let meeting_count = export_result.metadata_value("meeting_count").unwrap_or("1");
    let meeting_noun = if meeting_count == "1" {
        "meeting"
    } else {
        "meetings"
    };
    format!(
        "Exported {meeting_count} {meeting_noun} as {} to {} ({} bytes)",
        export_result.format(),
        export_result.file_path(),
        export_result.size_bytes()
    )
}

/// Holds a nested meeting action for clap.
#[derive(Debug, Args)]
pub struct MeetingArgs {
    /// Selects the meeting action.
    #[command(subcommand)]
    pub command: MeetingCommand,
}

/// Defines voice-gallery actions.
#[derive(Debug, Subcommand)]
pub enum VoiceCommand {
    List,
    Assign {
        clip_id: Uuid,
        identity_id: Uuid,
    },
    /// Lists a meeting's unresolved unknown-speaker clusters.
    ListClusters {
        /// Meeting whose clusters are listed.
        meeting_id: String,
    },
    /// Assigns every chunk of an unknown-speaker cluster to a named identity.
    AssignCluster {
        /// Cluster UUID printed by `voice list-clusters`.
        cluster_id: Uuid,
        /// Name of the person; a known identity with this name is reused, otherwise one is created.
        identity_name: String,
    },
}

/// Holds a nested voice action for clap.
#[derive(Debug, Args)]
pub struct VoiceArgs {
    /// Selects the voice action.
    #[command(subcommand)]
    pub command: VoiceCommand,
}

/// Provides the narrow service surface driven by parsed CLI commands.
pub trait CliService {
    /// Starts recording.
    fn start_recording(&mut self) -> Result<String, String>;
    /// Stops recording.
    fn stop_recording(&mut self) -> Result<String, String>;
    /// Reports recording state.
    fn recording_status(&self) -> Result<String, String>;
    /// Loads terminal-safe meeting rows.
    fn list_meetings(&self) -> Result<Vec<(String, String, String)>, String>;
    /// Loads one terminal-safe meeting detail view.
    fn view_meeting(&self, meeting_id: &str) -> Result<String, String>;
    /// Loads display-safe voice rows.
    fn list_voices(&self) -> Result<Vec<(String, String, usize)>, String>;
    /// Resolves a clip through the voice identity application service.
    fn assign_voice_clip(&mut self, clip_id: Uuid, identity_id: Uuid) -> Result<String, String>;
    /// Loads `(cluster_id, total_duration_ms, chunk_count)` rows for a meeting's unresolved
    /// unknown-speaker clusters.
    fn list_unknown_clusters(&self, meeting_id: &str) -> Result<Vec<(String, u64, usize)>, String>;
    /// Assigns a cluster to the identity named `identity_name` and describes the outcome.
    fn assign_cluster(&mut self, cluster_id: Uuid, identity_name: &str) -> Result<String, String>;
    /// Loads aggregate metrics.
    fn telemetry(&self) -> Result<String, String>;
    /// Exports one meeting through the shared export manager.
    fn export_meeting(
        &self,
        meeting_id: &str,
        options: &ExportOptions,
        output: Option<&Path>,
    ) -> Result<String, String>;
    /// Exports every meeting matching the options' date range into one file.
    fn export_all(&self, options: &ExportOptions, output: Option<&Path>) -> Result<String, String>;
}

/// Dispatches a clap-validated command to an injected service.
///
/// Inputs: A parsed `command` and a port/application-backed `cli_service`.
/// Outputs: Safe human-readable output or a portable error string.
/// Side effects: Delegates only the selected service operation.
pub fn dispatch(command: CliCommand, cli_service: &mut impl CliService) -> Result<String, String> {
    match command {
        CliCommand::Record(RecordArgs {
            command: RecordCommand::Start,
        }) => cli_service.start_recording(),
        CliCommand::Record(RecordArgs {
            command: RecordCommand::Stop,
        }) => cli_service.stop_recording(),
        CliCommand::Record(RecordArgs {
            command: RecordCommand::Status,
        }) => cli_service.recording_status(),
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::List,
        }) => Ok(format_meeting_table(cli_service.list_meetings()?)),
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::View { meeting_id },
        }) => cli_service.view_meeting(&meeting_id),
        CliCommand::Meetings(MeetingArgs {
            command:
                MeetingCommand::Export {
                    meeting_id,
                    export_arguments,
                },
        }) => cli_service.export_meeting(
            &meeting_id,
            &export_arguments.export_options(None),
            export_arguments.output.as_deref(),
        ),
        CliCommand::Meetings(MeetingArgs {
            command:
                MeetingCommand::ExportAll {
                    date_range,
                    export_arguments,
                },
        }) => cli_service.export_all(
            &export_arguments.export_options(date_range),
            export_arguments.output.as_deref(),
        ),
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::List,
        }) => Ok(format_voice_table(cli_service.list_voices()?)),
        CliCommand::Voice(VoiceArgs {
            command:
                VoiceCommand::Assign {
                    clip_id,
                    identity_id,
                },
        }) => cli_service.assign_voice_clip(clip_id, identity_id),
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::ListClusters { meeting_id },
        }) => Ok(format_cluster_table(
            &meeting_id,
            cli_service.list_unknown_clusters(&meeting_id)?,
        )),
        CliCommand::Voice(VoiceArgs {
            command:
                VoiceCommand::AssignCluster {
                    cluster_id,
                    identity_name,
                },
        }) => cli_service.assign_cluster(cluster_id, &identity_name),
        CliCommand::Telemetry => cli_service.telemetry(),
    }
}

/// Formats meeting rows as a stable terminal table.
fn format_meeting_table(meetings: Vec<(String, String, String)>) -> String {
    std::iter::once(String::from("ID | Date | Duration"))
        .chain(
            meetings
                .into_iter()
                .map(|(meeting_id, date, duration)| format!("{meeting_id} | {date} | {duration}")),
        )
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats identity rows without embedding data.
fn format_voice_table(voices: Vec<(String, String, usize)>) -> String {
    std::iter::once(String::from("ID | Name | Variants"))
        .chain(
            voices.into_iter().map(|(identity_id, name, variants)| {
                format!("{identity_id} | {name} | {variants}")
            }),
        )
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats unknown-speaker cluster rows, or a sentence when the meeting has none.
///
/// Inputs: `meeting_id` for the empty message; `(cluster_id, total_duration_ms, chunk_count)` rows.
/// Outputs: A `Cluster ID | Total Duration | Chunks` table with durations in seconds.
/// Side effects: None.
fn format_cluster_table(meeting_id: &str, clusters: Vec<(String, u64, usize)>) -> String {
    if clusters.is_empty() {
        return format!("No unresolved unknown-speaker clusters for meeting {meeting_id}");
    }
    std::iter::once(String::from("Cluster ID | Total Duration | Chunks"))
        .chain(
            clusters
                .into_iter()
                .map(|(cluster_id, total_duration_ms, chunk_count)| {
                    format!(
                        "{cluster_id} | {} | {chunk_count}",
                        format_seconds(total_duration_ms)
                    )
                }),
        )
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats milliseconds as seconds rounded half-up to one decimal, such as `3.5s`.
fn format_seconds(duration_ms: u64) -> String {
    let tenths_of_second = duration_ms.saturating_add(50) / 100;
    format!("{}.{}s", tenths_of_second / 10, tenths_of_second % 10)
}

/// Describes a completed cluster assignment for the terminal.
fn format_cluster_assignment(assignment: &NamedClusterAssignment) -> String {
    format!(
        "Assigned cluster {} to \"{}\" ({} {}; {} voice variants stored, {} transcript segments updated)",
        assignment.cluster_id(),
        assignment.identity_name(),
        if assignment.identity_created() {
            "new identity"
        } else {
            "existing identity"
        },
        assignment.identity_id(),
        assignment.variants_stored(),
        assignment.segments_updated(),
    )
}

/// Adapts existing portable recorder, repository, and voice-manager services for the CLI.
pub struct CoreCliService<RecorderPort, RepositoryPort, VectorDatabasePort> {
    recorder: RecorderPort,
    repository: RepositoryPort,
    voice_identity_manager: VoiceIdentityManager<VectorDatabasePort>,
    export_writer: ExportWriter,
}

impl<RecorderPort, RepositoryPort, VectorDatabasePort>
    CoreCliService<RecorderPort, RepositoryPort, VectorDatabasePort>
where
    VectorDatabasePort: VoiceVectorDB,
{
    /// Composes a CLI service from existing adapters and the voice identity application service.
    ///
    /// Inputs: A recorder port, repository port, vector database port, and the export writer
    /// (exporters plus default export directory) composed by the binary.
    /// Outputs: A driving adapter service with no SQLite or business-policy implementation.
    /// Side effects: Takes ownership of supplied dependencies without calling them.
    pub fn new(
        recorder: RecorderPort,
        repository: RepositoryPort,
        vector_database: VectorDatabasePort,
        export_writer: ExportWriter,
    ) -> Self {
        Self {
            recorder,
            repository,
            voice_identity_manager: VoiceIdentityManager::new(vector_database),
            export_writer,
        }
    }

    /// Builds an export manager over this service's ports for one command.
    fn export_manager(
        &self,
    ) -> ExportManager<'_, RepositoryPort, RepositoryPort, VectorDatabasePort>
    where
        RepositoryPort: NoteRepository + AttachmentRepository,
    {
        ExportManager::new(
            &self.repository,
            &self.repository,
            self.voice_identity_manager.vector_database(),
            &self.export_writer,
        )
    }
}

impl<RecorderPort, RepositoryPort, VectorDatabasePort> CliService
    for CoreCliService<RecorderPort, RepositoryPort, VectorDatabasePort>
where
    RecorderPort: AudioCapture,
    RepositoryPort: NoteRepository + AttachmentRepository + MetricsRepository,
    VectorDatabasePort: VoiceVectorDB,
{
    fn start_recording(&mut self) -> Result<String, String> {
        self.recorder
            .start()
            .map(|()| String::from("recording started"))
            .map_err(|error| error.to_string())
    }
    fn stop_recording(&mut self) -> Result<String, String> {
        self.recorder
            .stop()
            .map(|()| String::from("recording stopped"))
            .map_err(|error| error.to_string())
    }
    fn recording_status(&self) -> Result<String, String> {
        Ok(format!("{:?}", self.recorder.current_state()))
    }
    fn list_meetings(&self) -> Result<Vec<(String, String, String)>, String> {
        self.repository
            .list_meeting_ids()
            .map(|meeting_ids| {
                meeting_ids
                    .into_iter()
                    .map(|meeting_id| {
                        (
                            String::from(meeting_id.as_str()),
                            String::from("unknown"),
                            String::from("unknown"),
                        )
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    }
    fn view_meeting(&self, meeting_id: &str) -> Result<String, String> {
        let typed_meeting_id = MeetingId::new(meeting_id);
        let transcript = self
            .repository
            .load_transcript(&typed_meeting_id)
            .map_err(|error| error.to_string())?
            .map(|value| String::from(value.content()))
            .unwrap_or_else(|| String::from("(no transcript)"));
        let attachments = self
            .repository
            .get_attachments_for_meeting(&typed_meeting_id)
            .map_err(|error| error.to_string())?;
        let attachment_lines = attachments
            .into_iter()
            .map(|attachment| format!("- {} ({})", attachment.file_path(), attachment.mime_type()))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(format!(
            "Meeting: {meeting_id}\nTranscript:\n{transcript}\nAttachments:\n{attachment_lines}"
        ))
    }
    fn list_voices(&self) -> Result<Vec<(String, String, usize)>, String> {
        self.voice_identity_manager
            .list_identities_with_variant_counts()
            .map(|identities| {
                identities
                    .into_iter()
                    .map(|(identity, count)| {
                        (
                            identity.identity_id().to_string(),
                            String::from(identity.display_name()),
                            count,
                        )
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    }
    fn assign_voice_clip(&mut self, clip_id: Uuid, identity_id: Uuid) -> Result<String, String> {
        let meeting_ids = self
            .repository
            .list_meeting_ids()
            .map_err(|error| error.to_string())?;
        let mut found_clip = None;
        for meeting_id in meeting_ids {
            let clips = self
                .repository
                .get_unassigned_clips(&meeting_id)
                .map_err(|error| error.to_string())?;
            if let Some(clip) = clips.into_iter().find(|clip| clip.clip_id() == clip_id) {
                found_clip = Some(clip);
                break;
            }
        }
        let clip = found_clip.ok_or_else(|| String::from("unassigned clip was not found"))?;
        self.voice_identity_manager
            .confirm_weak_match(identity_id, clip.derived_embedding())
            .map(|_| String::from("voice clip assigned"))
            .map_err(|error| error.to_string())
    }
    fn list_unknown_clusters(&self, meeting_id: &str) -> Result<Vec<(String, u64, usize)>, String> {
        list_unknown_clusters(&self.repository, &MeetingId::new(meeting_id))
            .map(|summaries| {
                summaries
                    .iter()
                    .map(|summary| {
                        (
                            summary.cluster_id().to_string(),
                            summary.total_duration_ms(),
                            summary.chunk_count(),
                        )
                    })
                    .collect()
            })
            .map_err(|error| error.to_string())
    }
    fn assign_cluster(&mut self, cluster_id: Uuid, identity_name: &str) -> Result<String, String> {
        assign_cluster_to_named_identity(
            &self.repository,
            self.voice_identity_manager.vector_database(),
            cluster_id,
            identity_name,
        )
        .map(|assignment| format_cluster_assignment(&assignment))
        .map_err(|error| error.to_string())
    }
    fn telemetry(&self) -> Result<String, String> {
        self.repository
            .get_aggregated_stats()
            .map(|stats| {
                format!(
                    "Total transcription time: {} ms\nAverage latency: {} ms",
                    stats.total_duration_ms(),
                    stats.average_latency_ms()
                )
            })
            .map_err(|error| error.to_string())
    }
    fn export_meeting(
        &self,
        meeting_id: &str,
        options: &ExportOptions,
        output: Option<&Path>,
    ) -> Result<String, String> {
        self.export_manager()
            .export_meeting(&MeetingId::new(meeting_id), options, output)
            .map(|export_result| format_export_result(&export_result))
            .map_err(|error| format!("{error:#}"))
    }
    fn export_all(&self, options: &ExportOptions, output: Option<&Path>) -> Result<String, String> {
        self.export_manager()
            .export_all(options, output)
            .map(|export_result| format_export_result(&export_result))
            .map_err(|error| format!("{error:#}"))
    }
}

#[cfg(test)]
mod tests;
