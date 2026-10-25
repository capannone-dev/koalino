//! Defines the CLI driving-adapter boundary and its test-first command contract.
//!
//! Command parsing and terminal formatting stay here while domain policy and persistence remain
//! behind existing application services and repository ports.

use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

use crate::{
    application::voice_identity_manager::VoiceIdentityManager,
    domain::models::MeetingId,
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

/// Defines meeting read actions.
#[derive(Debug, Subcommand)]
pub enum MeetingCommand {
    List,
    View { meeting_id: String },
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
    Assign { clip_id: Uuid, identity_id: Uuid },
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
    /// Loads aggregate metrics.
    fn telemetry(&self) -> Result<String, String>;
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

/// Adapts existing portable recorder, repository, and voice-manager services for the CLI.
pub struct CoreCliService<RecorderPort, RepositoryPort, VectorDatabasePort> {
    recorder: RecorderPort,
    repository: RepositoryPort,
    voice_identity_manager: VoiceIdentityManager<VectorDatabasePort>,
}

impl<RecorderPort, RepositoryPort, VectorDatabasePort>
    CoreCliService<RecorderPort, RepositoryPort, VectorDatabasePort>
where
    VectorDatabasePort: VoiceVectorDB,
{
    /// Composes a CLI service from existing adapters and the voice identity application service.
    ///
    /// Inputs: A recorder port, repository port, and vector database port.
    /// Outputs: A driving adapter service with no SQLite or business-policy implementation.
    /// Side effects: Takes ownership of supplied dependencies without calling them.
    pub fn new(
        recorder: RecorderPort,
        repository: RepositoryPort,
        vector_database: VectorDatabasePort,
    ) -> Self {
        Self {
            recorder,
            repository,
            voice_identity_manager: VoiceIdentityManager::new(vector_database),
        }
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
}

#[cfg(test)]
mod tests;
