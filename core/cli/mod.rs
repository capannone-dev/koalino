//! Defines the CLI driving-adapter boundary and its test-first command contract.
//!
//! Command parsing and terminal formatting stay here while domain policy and persistence remain
//! behind existing application services and repository ports.

use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, NaiveTime};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

use crate::adapters::observability::LogLevelControlFile;
use crate::domain::transcription_glossary::TranscriptionGlossary;
use crate::{
    application::{
        cluster_review::{
            NamedClusterAssignment, assign_cluster_to_named_identity, list_unknown_clusters,
        },
        combined_meetings::{MeetingListing, split_recordings},
        deletion::{self, MeetingDeletionHooks},
        export_manager::{ExportManager, ExportWriter},
        meeting_rename::rename_meeting,
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::{
        models::{
            DeviceId, ExportDateRange, ExportFormat, ExportOptions, ExportResult, MeetingId,
            MeetingMetadata, MeetingTitle,
        },
        peer_trust::TrustedPeer,
        transcription_language::TranscriptionLanguage,
    },
    ports::{
        peer_trust::TrustedPeerStore,
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
    /// Verbosity of this command's own logs (`error`, `warn`, `info`, `debug`, `trace`, or a
    /// `tracing` directive). Without it the log file records `info` and stdout shows only
    /// errors, so JSON logs do not mix with command output.
    #[arg(long, global = true, value_name = "LEVEL")]
    pub log_level: Option<String>,
}

/// The tracing filters for the CLI's two log sinks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliLogFilters {
    /// Filter for the rolling JSON log file.
    pub file_filter: String,
    /// Filter for JSON logs on stdout, which is shared with command output.
    pub stdout_filter: String,
}

impl Cli {
    /// Chooses the log filters for this invocation.
    ///
    /// Inputs: `self.log_level`, the optional `--log-level` value.
    /// Outputs: The requested level for both sinks; otherwise `info` for the file (the
    /// production default of REQUIREMENTS 6.1) and `error` for stdout.
    /// Side effects: None.
    pub fn log_filters(&self) -> CliLogFilters {
        match &self.log_level {
            Some(requested_level) => CliLogFilters {
                file_filter: requested_level.clone(),
                stdout_filter: requested_level.clone(),
            },
            None => CliLogFilters {
                file_filter: String::from("info"),
                stdout_filter: String::from("error"),
            },
        }
    }
}

/// Defines top-level command groups.
#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// Controls portable audio recording.
    Record(RecordArgs),
    /// Reads, renames, and exports locally persisted meetings.
    Meetings(MeetingArgs),
    /// Lists or resolves voice review items.
    Voice(VoiceArgs),
    /// Lists or removes the devices paired for sync.
    Devices(DevicesArgs),
    /// Shows or changes the names and terms transcription is prompted with, so they are spelled
    /// right; it applies from the next recording, on every app using this database.
    Glossary(GlossaryArgs),
    /// Shows or sets how many days a recording's speech may wait to be combined with another
    /// device's recording of the same meeting (1 to 60; three weeks by default). Speech older
    /// than a shorter window is deleted at the next round.
    KeepSpeech {
        /// The new window in days; omit it to show the current one.
        days: Option<u16>,
    },
    /// Shows or switches whether an online-meeting participant's voice is kept in the voice
    /// gallery after enough speech of that one voice (off by default; voice prints are biometric).
    /// A meeting room or other track with several voices is never kept.
    MeetingVoices {
        /// `on` or `off`; omit it to show the current choice.
        choice: Option<OnOff>,
    },
    /// Writes a meeting's summary with an AI connection (the default unless `--connection` names
    /// one), or shows the stored one with `--show`. The meeting's transcript, notes and photos
    /// are sent to that connection's server.
    Summary {
        /// The meeting to summarize.
        meeting_id: String,
        /// The connection to use, by ID (see `ai list`).
        #[arg(long)]
        connection: Option<String>,
        /// Shows the stored summary instead of writing a new one.
        #[arg(long)]
        show: bool,
    },
    /// Lists, adds and removes the AI connections summaries use, and turns automatic summaries
    /// on or off.
    Ai(AiArgs),
    /// Finds where words were said or written: every meeting's title, summary, notes and
    /// transcript, or with `--projects` / `--project` the projects' names, files and meetings.
    /// Case and accents are ignored; every word must be in the same passage.
    Search {
        /// The words to find.
        #[arg(required = true, num_args = 1..)]
        words: Vec<String>,
        /// Searches every project instead of every meeting.
        #[arg(long, conflicts_with = "project")]
        projects: bool,
        /// Searches one project, by ID (see `projects` in the app or the web).
        #[arg(long)]
        project: Option<String>,
    },
    /// Asks the default AI connection about your notes: every meeting, summary and project, or
    /// one meeting or project. The chat is kept on this device, sealed, and each question carries
    /// the recent ones. The answer says what the model was given.
    Chat {
        /// The question.
        words: Vec<String>,
        /// Talks about one meeting.
        #[arg(long, conflicts_with = "project")]
        meeting: Option<String>,
        /// Talks about one project, by ID.
        #[arg(long)]
        project: Option<String>,
        /// Uses this connection instead of the default one.
        #[arg(long)]
        connection: Option<String>,
        /// Prints the chat so far instead of asking.
        #[arg(long, conflicts_with = "clear")]
        history: bool,
        /// Deletes the chat.
        #[arg(long)]
        clear: bool,
    },
    /// Shows, turns on or off, or syncs now the notes folder: plain files mirroring projects
    /// and meetings, editable with any editor (#98). The folder is a decrypted copy, so it is off
    /// until turned on.
    NotesFolder {
        #[command(subcommand)]
        action: Option<NotesFolderAction>,
    },
    /// Displays aggregate privacy-safe telemetry.
    Telemetry,
    /// Changes a running daemon's log level without restarting it (applied within seconds).
    LogLevel {
        /// `error`, `warn`, `info`, `debug`, `trace`, or a `tracing` directive such as
        /// `note_taker_core=debug,info`.
        directive: String,
    },
}

/// Wraps the AI connection actions.
#[derive(Debug, Args)]
pub struct AiArgs {
    /// The action.
    #[command(subcommand)]
    pub command: AiCommand,
}

/// A switch given on the command line.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum OnOff {
    /// Switch on.
    On,
    /// Switch off.
    Off,
}

/// The protocol of a new AI connection.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum AiKind {
    /// OpenAI's Chat Completions, also spoken by Mistral, Ollama, llama.cpp, LM Studio and vLLM.
    Openai,
    /// Anthropic's Messages API.
    Anthropic,
}

/// AI connection actions (#95).
#[derive(Debug, Subcommand)]
pub enum AiCommand {
    /// Lists the connections, the default one and whether summaries are automatic.
    List,
    /// Adds a connection. Its API key, if it needs one, is read from `NOTE_TAKER_LLM_API_KEY`,
    /// so it never lands in the shell history, and kept in the system keychain.
    Add {
        /// A name for it, such as `Office server`.
        #[arg(long)]
        label: String,
        /// The protocol it speaks.
        #[arg(long, value_enum)]
        kind: AiKind,
        /// Its address: HTTPS, or plain HTTP on this machine or the local network.
        #[arg(long)]
        url: String,
        /// The model, such as `llama3.1:8b` or `claude-opus-5-5`.
        #[arg(long)]
        model: String,
        /// How many tokens the model reads at once.
        #[arg(long, default_value_t = 32_768)]
        context: u32,
    },
    /// Removes a connection and its key.
    Remove {
        /// The connection's ID.
        id: String,
    },
    /// Makes a connection the default.
    Default {
        /// The connection's ID.
        id: String,
    },
    /// Sends a one-line test request, holding no meeting content.
    Test {
        /// The connection's ID.
        id: String,
    },
    /// Turns automatic summaries on or off.
    Auto {
        /// `on` or `off`.
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
}

/// Defines recording state actions.
#[derive(Debug, Subcommand)]
pub enum RecordCommand {
    /// Records into a meeting in the foreground until the input ends, Ctrl+C, or `record stop`,
    /// then waits until every chunk is transcribed.
    Start {
        /// Meeting to record into (created when missing); a new ID is generated by default.
        #[arg(long)]
        meeting_id: Option<String>,
        /// Human-readable meeting title; omitted titles show as `Meeting <creation time>`, and an
        /// existing meeting keeps its current title.
        #[arg(long)]
        title: Option<String>,
        /// A 16 kHz mono 16-bit PCM WAV file, or `-` for raw little-endian PCM on stdin (e.g.
        /// `arecord -f S16_LE -r 16000 -c 1 -t raw | note-taker record start --input -`).
        /// Without it, the default microphone is recorded (builds with the `microphone` feature).
        #[arg(long)]
        input: Option<PathBuf>,
        /// Language spoken in the meeting: `auto` (detect it) or a Whisper code such as `it` or
        /// `en`. Defaults to `NOTE_TAKER_WHISPER_LANGUAGE`, else `auto`. Kept in the meeting's
        /// details either way.
        #[arg(long)]
        language: Option<String>,
    },
    /// Asks the running `record start` to finish and waits until its audio is transcribed.
    Stop,
    /// Reports whether a recording is running.
    Status,
}

/// Holds a nested recording action for clap.
#[derive(Debug, Args)]
pub struct RecordArgs {
    /// Selects the recording action.
    #[command(subcommand)]
    pub command: RecordCommand,
}

/// Defines meeting read, rename, and export actions.
#[derive(Debug, Subcommand)]
pub enum MeetingCommand {
    /// Lists meetings with their titles, creation dates, and durations.
    List,
    /// Shows one meeting's title, transcript, and attachments.
    View {
        /// The meeting to show.
        meeting_id: String,
    },
    /// Changes a meeting's title and prints the old and new titles.
    Rename {
        /// The meeting to rename.
        meeting_id: String,
        /// The new title: 1–200 characters after trimming, on one line, without control
        /// characters. Quote it when it contains spaces.
        new_title: String,
    },
    /// Deletes a meeting with its transcript, attachments and unknown voices, here and on paired
    /// devices. This cannot be undone.
    Delete {
        /// The meeting to delete.
        meeting_id: String,
        /// Confirms the deletion; without it nothing is deleted.
        #[arg(long)]
        yes: bool,
    },
    /// Lists the other devices' recordings combined into a meeting on their own again, for good.
    /// Pass a combined recording's ID to split only that one.
    Split {
        /// The listed meeting, or one recording combined into it.
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
    /// Deletes an unknown voice here and on paired devices; its transcript stays. This cannot be
    /// undone.
    DeleteCluster {
        /// Cluster UUID printed by `voice list-clusters`.
        cluster_id: Uuid,
        /// Confirms the deletion; without it nothing is deleted.
        #[arg(long)]
        yes: bool,
    },
    /// Deletes a saved person's name and voice samples here and on paired devices; transcripts stay
    /// and show an unknown speaker. This cannot be undone.
    DeletePerson {
        /// Identity UUID printed by `voice list`.
        identity_id: Uuid,
        /// Confirms the deletion; without it nothing is deleted.
        #[arg(long)]
        yes: bool,
    },
}

/// Holds a nested voice action for clap.
#[derive(Debug, Args)]
pub struct VoiceArgs {
    /// Selects the voice action.
    #[command(subcommand)]
    pub command: VoiceCommand,
}

/// Defines paired-device actions.
#[derive(Debug, Subcommand)]
pub enum DevicesCommand {
    /// Lists the devices this database syncs with.
    List,
    /// Removes a paired device; a running daemon on this database stops syncing with it within a
    /// second. Pair again from a Devices screen to resume.
    Revoke {
        /// The device's peer ID, or its name when no other paired device shares it.
        device: String,
    },
}

/// Defines glossary actions.
#[derive(Debug, Subcommand)]
pub enum GlossaryCommand {
    /// Prints the terms, one per line.
    Show,
    /// Replaces the glossary with these terms.
    Set {
        /// Names, project names, acronyms or jargon; quote a term with spaces.
        terms: Vec<String>,
    },
    /// Adds terms; a term already there (in any case) is kept once.
    Add {
        /// The terms to add.
        #[arg(required = true)]
        terms: Vec<String>,
    },
    /// Removes terms, ignoring case.
    Remove {
        /// The terms to remove.
        #[arg(required = true)]
        terms: Vec<String>,
    },
    /// Removes every term.
    Clear,
}

/// Holds a nested glossary action for clap.
#[derive(Debug, Args)]
pub struct GlossaryArgs {
    /// Selects the glossary action.
    #[command(subcommand)]
    pub command: GlossaryCommand,
}

/// Holds a nested paired-device action for clap.
#[derive(Debug, Args)]
pub struct DevicesArgs {
    /// Selects the paired-device action.
    #[command(subcommand)]
    pub command: DevicesCommand,
}

/// One terminal-safe row of `devices list`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedDeviceRow {
    /// The device's libp2p peer ID.
    pub peer_id: String,
    /// The name the device announced when it paired.
    pub device_name: String,
    /// The UTC pairing time, formatted `YYYY-MM-DD HH:MM UTC`.
    pub paired_at: String,
}

/// One terminal-safe row of `meetings list`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingListRow {
    /// The stable meeting identifier.
    pub meeting_id: String,
    /// The stored title, or the dated fallback for an untitled meeting.
    pub title: String,
    /// The UTC creation time, formatted `YYYY-MM-DD HH:MM UTC`.
    pub created_at: String,
    /// The end of the latest transcript segment, formatted `M:SS` or `H:MM:SS`.
    pub duration: String,
    /// How many devices recorded the meeting: more than 1 when recordings were combined (#88).
    pub device_count: usize,
}

/// Runs foreground recordings for the CLI; implemented by the binary's composition root.
pub trait RecordingControl {
    /// Records `input` into a meeting and returns a summary once all audio is transcribed.
    ///
    /// Inputs: The meeting (`None` generates one), an optional title stored on it before audio
    /// is captured, the input path (`-` for stdin; `None` for the default microphone), and the
    /// language spoken (`None` for the configured default).
    /// Outputs: A terminal-safe summary, or a readable error.
    /// Side effects: Blocks for the whole recording; persists the title, transcript segments and
    /// the language used in the meeting's metadata.
    fn record(
        &mut self,
        meeting_id: Option<MeetingId>,
        title: Option<MeetingTitle>,
        input: Option<&Path>,
        language: Option<TranscriptionLanguage>,
    ) -> Result<String, String>;
    /// Asks a running recording (in any process) to stop, and waits until it has.
    fn request_stop(&mut self) -> Result<String, String>;
    /// Describes whether a recording is running.
    fn status(&self) -> Result<String, String>;
}

/// Refuses a deletion that was not confirmed with `--yes`.
///
/// Outputs: Success, or the message to show. Side effects: None.
fn confirmed(yes: bool) -> Result<(), String> {
    if yes {
        Ok(())
    } else {
        Err(String::from(
            "this deletes it for good, here and on paired devices; add --yes to confirm",
        ))
    }
}

/// Provides the narrow service surface driven by parsed CLI commands.
pub trait CliService {
    /// Records `input` (`None`: the microphone) into a meeting, optionally titled and in a
    /// chosen language (`None`: the configured default), in the foreground.
    fn start_recording(
        &mut self,
        meeting_id: Option<&str>,
        title: Option<&str>,
        input: Option<&Path>,
        language: Option<&str>,
    ) -> Result<String, String>;
    /// Stops recording.
    fn stop_recording(&mut self) -> Result<String, String>;
    /// Reports recording state.
    fn recording_status(&self) -> Result<String, String>;
    /// Loads terminal-safe meeting rows.
    fn list_meetings(&self) -> Result<Vec<MeetingListRow>, String>;
    /// Loads one terminal-safe meeting detail view.
    fn view_meeting(&self, meeting_id: &str) -> Result<String, String>;
    /// Replaces a meeting's title and describes the change.
    fn rename_meeting(&mut self, meeting_id: &str, new_title: &str) -> Result<String, String>;
    /// Loads display-safe voice rows.
    /// Deletes a meeting and everything it holds, and describes what happened.
    fn delete_meeting(&mut self, meeting_id: &str) -> Result<String, String>;
    /// Splits recordings combined into a meeting apart again, and describes what happened.
    fn split_meeting(&mut self, meeting_id: &str) -> Result<String, String>;
    /// Deletes an unknown voice and describes what happened.
    fn delete_unknown_voice(&mut self, cluster_id: Uuid) -> Result<String, String>;
    /// Deletes a saved person and describes what happened.
    fn delete_saved_person(&mut self, identity_id: Uuid) -> Result<String, String>;
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
    /// Asks a running daemon on the same database to switch to `filter_directive`.
    fn request_log_level(&mut self, filter_directive: &str) -> Result<String, String>;
    /// Loads the paired devices, oldest pairing first.
    fn list_paired_devices(&self) -> Result<Vec<PairedDeviceRow>, String>;
    /// Removes the paired device whose peer ID, or unique name, is `device`, and describes it.
    fn revoke_device(&mut self, device: &str) -> Result<String, String>;
    /// Loads the saved glossary's terms, in order.
    fn glossary(&self) -> Result<Vec<String>, String>;
    /// Saves the glossary's terms, already validated; an empty list clears it.
    fn save_glossary(&mut self, terms: &[String]) -> Result<(), String>;
    /// Loads how many days speech waits to be combined (#93).
    fn keep_speech_days(&self) -> Result<u16, String>;
    /// Saves that window, already checked to be in range.
    fn save_keep_speech_days(&mut self, days: u16) -> Result<(), String>;
    /// Loads whether online-meeting participants' voices are kept (#103).
    fn keeps_meeting_voices(&self) -> Result<bool, String>;
    /// Saves that choice.
    fn save_keep_meeting_voices(&mut self, keep: bool) -> Result<(), String>;
    /// Exports one meeting through the shared export manager.
    fn export_meeting(
        &self,
        meeting_id: &str,
        options: &ExportOptions,
        output: Option<&Path>,
    ) -> Result<String, String>;
    /// Exports every meeting matching the options' date range into one file.
    fn export_all(&self, options: &ExportOptions, output: Option<&Path>) -> Result<String, String>;
    /// Writes a meeting's summary and returns it (#95).
    fn summarize_meeting(
        &mut self,
        _meeting_id: &str,
        _connection: Option<&str>,
    ) -> Result<String, String> {
        Err(String::from("summaries are not available here"))
    }
    /// Returns a meeting's stored summary, or `None`.
    fn meeting_summary(&self, _meeting_id: &str) -> Result<Option<String>, String> {
        Err(String::from("summaries are not available here"))
    }
    /// Runs one AI settings command (see `llm_commands.rs`) and returns its answer.
    fn llm_command(&mut self, _request: &serde_json::Value) -> Result<serde_json::Value, String> {
        Err(String::from("AI connections are not available here"))
    }
    /// Runs a chat command (see `notes_chat_commands.rs`) and returns its answer (#97).
    fn chat(&mut self, _request: &serde_json::Value) -> Result<serde_json::Value, String> {
        Err(String::from("chat is not available here"))
    }
    /// Runs a notes folder command (see `notes_folder_commands.rs`) and returns its answer.
    fn notes_folder(&mut self, _request: &serde_json::Value) -> Result<serde_json::Value, String> {
        Err(String::from("the notes folder is not available here"))
    }
    /// Runs a search (see `search_commands.rs`) and returns its JSON answer (#100).
    fn search(
        &self,
        _request: &crate::search_commands::SearchRequest,
    ) -> Result<serde_json::Value, String> {
        Err(String::from("search is not available here"))
    }
}

/// Dispatches a clap-validated command to an injected service.
///
/// Inputs: A parsed `command` and a port/application-backed `cli_service`.
/// Outputs: Safe human-readable output or a portable error string.
/// Side effects: Delegates only the selected service operation.
pub fn dispatch(command: CliCommand, cli_service: &mut impl CliService) -> Result<String, String> {
    match command {
        CliCommand::Record(RecordArgs {
            command:
                RecordCommand::Start {
                    meeting_id,
                    title,
                    input,
                    language,
                },
        }) => cli_service.start_recording(
            meeting_id.as_deref(),
            title.as_deref(),
            input.as_deref(),
            language.as_deref(),
        ),
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
                MeetingCommand::Rename {
                    meeting_id,
                    new_title,
                },
        }) => cli_service.rename_meeting(&meeting_id, &new_title),
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::Delete { meeting_id, yes },
        }) => confirmed(yes).and_then(|()| cli_service.delete_meeting(&meeting_id)),
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::Split { meeting_id },
        }) => cli_service.split_meeting(&meeting_id),
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::DeleteCluster { cluster_id, yes },
        }) => confirmed(yes).and_then(|()| cli_service.delete_unknown_voice(cluster_id)),
        CliCommand::Voice(VoiceArgs {
            command: VoiceCommand::DeletePerson { identity_id, yes },
        }) => confirmed(yes).and_then(|()| cli_service.delete_saved_person(identity_id)),
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
        CliCommand::Devices(DevicesArgs {
            command: DevicesCommand::List,
        }) => Ok(format_device_table(cli_service.list_paired_devices()?)),
        CliCommand::Devices(DevicesArgs {
            command: DevicesCommand::Revoke { device },
        }) => cli_service.revoke_device(&device),
        CliCommand::Glossary(GlossaryArgs { command }) => edit_glossary(command, cli_service),
        CliCommand::KeepSpeech { days } => keep_speech(days, cli_service),
        CliCommand::MeetingVoices { choice } => meeting_voices(choice, cli_service),
        CliCommand::Summary {
            meeting_id,
            connection: _,
            show: true,
        } => Ok(cli_service
            .meeting_summary(&meeting_id)?
            .unwrap_or_else(|| String::from("This meeting has no summary yet."))),
        CliCommand::Summary {
            meeting_id,
            connection,
            show: false,
        } => cli_service.summarize_meeting(&meeting_id, connection.as_deref()),
        CliCommand::Ai(AiArgs { command }) => run_ai(command, cli_service),
        CliCommand::Search {
            words,
            projects,
            project,
        } => {
            let request = crate::search_commands::SearchRequest {
                query: words.join(" "),
                scope: Some(String::from(match (&project, projects) {
                    (Some(_), _) => "project",
                    (None, true) => "projects",
                    (None, false) => "meetings",
                })),
                project_id: project,
            };
            format_search(&cli_service.search(&request)?)
        }
        CliCommand::Chat {
            words,
            meeting,
            project,
            connection,
            history,
            clear,
        } => {
            let (scope, id) = match (meeting, project) {
                (Some(meeting), _) => ("meeting", Some(meeting)),
                (None, Some(project)) => ("project", Some(project)),
                (None, None) => ("all", None),
            };
            let command = if history {
                "history"
            } else if clear {
                "clear"
            } else {
                "ask"
            };
            if command == "ask" && words.is_empty() {
                return Err(String::from("type a question, or use --history or --clear"));
            }
            let request = serde_json::json!({
                "command": command,
                "scope": scope,
                "id": id,
                "question": words.join(" "),
                "connection_id": connection,
            });
            format_chat(&cli_service.chat(&request)?)
        }
        CliCommand::NotesFolder { action } => {
            let request = match action.unwrap_or(NotesFolderAction::Status) {
                NotesFolderAction::Status => serde_json::json!({"command": "get"}),
                NotesFolderAction::On { path } => {
                    serde_json::json!({"command": "set", "enabled": true, "path": path})
                }
                NotesFolderAction::Off => serde_json::json!({"command": "set", "enabled": false}),
                NotesFolderAction::Sync => serde_json::json!({"command": "sync_now"}),
            };
            format_notes_folder(&cli_service.notes_folder(&request)?)
        }
        CliCommand::Telemetry => cli_service.telemetry(),
        CliCommand::LogLevel { directive } => cli_service.request_log_level(&directive),
    }
}

/// Words a chat answer for the terminal: the answer and what it was given, the chat so far, or
/// whether it was cleared.
fn format_chat(answer: &serde_json::Value) -> Result<String, String> {
    if answer["ok"] != true {
        return Err(answer["error"]
            .as_str()
            .unwrap_or("the chat failed")
            .to_owned());
    }
    let list = |value: &serde_json::Value| -> Vec<String> {
        value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let describe = |message: &serde_json::Value| -> String {
        let mut text = message["text"].as_str().unwrap_or_default().to_owned();
        let included = list(&message["included"]);
        if !included.is_empty() {
            text.push_str(&format!("\n\nGiven: {}", included.join("; ")));
        }
        let omitted = list(&message["omitted"]);
        if !omitted.is_empty() {
            text.push_str(&format!("\nLeft out for length: {}", omitted.join("; ")));
        }
        text
    };
    if answer["answer"].is_object() {
        return Ok(describe(&answer["answer"]));
    }
    if let Some(messages) = answer["messages"].as_array() {
        if messages.is_empty() {
            return Ok(String::from("This chat is empty."));
        }
        return Ok(messages
            .iter()
            .map(|message| {
                let who = if message["role"] == "user" {
                    "You"
                } else {
                    "AI"
                };
                format!("{who}: {}", describe(message))
            })
            .collect::<Vec<_>>()
            .join("\n\n"));
    }
    Ok(if answer["cleared"] == true {
        String::from("Chat deleted.")
    } else {
        String::from("This chat was already empty.")
    })
}

/// What `notes-folder` does.
#[derive(Clone, Debug, Subcommand, PartialEq, Eq)]
pub enum NotesFolderAction {
    /// Shows whether the folder is on, where, and how the last round went (the default).
    Status,
    /// Turns the folder on and writes it now; without a path, `~/NoteTaker` or the saved one.
    On {
        /// The folder, absolute.
        path: Option<PathBuf>,
    },
    /// Turns the folder off; the files already written stay.
    Off,
    /// Reads edits back and writes the folder now.
    Sync,
}

/// Words a notes folder answer for the terminal.
///
/// Outputs: A line or two, or the refusal's message. Side effects: None.
fn format_notes_folder(answer: &serde_json::Value) -> Result<String, String> {
    if answer["ok"] != true {
        return Err(answer["error"]
            .as_str()
            .unwrap_or("the notes folder failed")
            .to_owned());
    }
    let path = answer["path"].as_str().unwrap_or("not chosen");
    let mut text = if answer["enabled"] == true {
        format!("Notes folder: on, in {path}")
    } else {
        String::from("Notes folder: off")
    };
    if let Some(at) = answer["last_round_at"].as_str() {
        match answer["last_error"].as_str() {
            Some(code) => text.push_str(&format!("\nLast round at {at} failed: {code}")),
            None => text.push_str(&format!(
                "\nLast round at {at}: {} files written, {} edits and files taken in",
                answer["written"], answer["imported"]
            )),
        }
    }
    Ok(text)
}

/// Words a search answer for the terminal: one line per hit, `[meeting ID] title · where:
/// snippet`, with the matched words in brackets.
///
/// Inputs: The JSON answer of `search_commands::run_search`. Outputs: The lines, `No results.`,
/// or the refusal's message. Side effects: None.
fn format_search(answer: &serde_json::Value) -> Result<String, String> {
    if answer["ok"] != true {
        return Err(answer["error"]
            .as_str()
            .unwrap_or("the search failed")
            .to_owned());
    }
    let hits = answer["hits"].as_array().cloned().unwrap_or_default();
    if hits.is_empty() {
        return Ok(String::from("No results."));
    }
    let text = |value: &serde_json::Value| value.as_str().unwrap_or_default().to_owned();
    let mut lines = vec![if answer["truncated"] == true {
        format!(
            "{} results (more not shown; add words to narrow the search)",
            hits.len()
        )
    } else if hits.len() == 1 {
        String::from("1 result")
    } else {
        format!("{} results", hits.len())
    }];
    for hit in &hits {
        let owner = if hit["meeting"].is_object() {
            format!(
                "[{}] {}",
                text(&hit["meeting"]["meeting_id"]),
                text(&hit["meeting"]["title"])
            )
        } else {
            format!("[project] {}", text(&hit["project"]["name"]))
        };
        let place = match hit["place"].as_str().unwrap_or_default() {
            "meeting_title" => String::from("title"),
            "summary" => String::from("summary"),
            "notes" => String::from("notes"),
            "transcript" => format!(
                "{} {}",
                format_offset(hit["start_time_ms"].as_u64().unwrap_or_default()),
                text(&hit["speaker"])
            ),
            "transcript_text" => String::from("transcript"),
            "project_name" => String::from("name"),
            "resource_name" => format!("file {}", text(&hit["resource"]["file_name"])),
            _ => format!("in {}", text(&hit["resource"]["file_name"])),
        };
        let snippet: String = hit["snippet"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .map(|part| {
                        if part["match"] == true {
                            format!("[{}]", text(&part["text"]))
                        } else {
                            text(&part["text"])
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        lines.push(format!("{owner} · {place}: {snippet}"));
    }
    Ok(lines.join("\n"))
}

/// Formats an offset in a meeting as `m:ss`, or `h:mm:ss` past an hour.
fn format_offset(offset_ms: u64) -> String {
    let seconds = offset_ms / 1_000;
    if seconds >= 3_600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3_600,
            (seconds / 60) % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// Carries out one glossary action.
///
/// Inputs: The action and the service holding the glossary.
/// Outputs: The terms (for `show`) or a confirmation; or why the new glossary was refused, in
/// which case nothing is saved.
/// Side effects: Reads and, except for `show`, saves the glossary.
fn edit_glossary(
    command: GlossaryCommand,
    cli_service: &mut impl CliService,
) -> Result<String, String> {
    let current = cli_service.glossary()?;
    let edited: Vec<String> = match command {
        GlossaryCommand::Show => {
            return Ok(if current.is_empty() {
                String::from("The glossary is empty.")
            } else {
                current.join("\n")
            });
        }
        GlossaryCommand::Set { terms } => terms,
        GlossaryCommand::Add { terms } => current.into_iter().chain(terms).collect(),
        GlossaryCommand::Remove { terms } => current
            .into_iter()
            .filter(|kept| {
                !terms
                    .iter()
                    .any(|term| term.trim().eq_ignore_ascii_case(kept))
            })
            .collect(),
        GlossaryCommand::Clear => Vec::new(),
    };
    let glossary = TranscriptionGlossary::parse(edited.iter().map(String::as_str))
        .map_err(|error| error.to_string())?;
    cli_service.save_glossary(glossary.terms())?;
    Ok(match glossary.terms().len() {
        0 => String::from("Glossary cleared."),
        1 => String::from("Glossary saved: 1 term. It applies from the next recording."),
        count => format!("Glossary saved: {count} terms. It applies from the next recording."),
    })
}

/// Carries out one AI connection action (#95).
///
/// Inputs: The action and the service. Outputs: A table or a confirmation, or the command's
/// error. Side effects: As the action says; `test` sends one request to the server.
fn run_ai(command: AiCommand, cli_service: &mut impl CliService) -> Result<String, String> {
    use serde_json::json;
    let request = match command {
        AiCommand::List => json!({"command": "settings"}),
        AiCommand::Add {
            label,
            kind,
            url,
            model,
            context,
        } => json!({
            "command": "save_connection",
            "label": label,
            "kind": match kind {
                AiKind::Openai => "open_ai_compatible",
                AiKind::Anthropic => "anthropic",
            },
            "base_url": url,
            "model": model,
            "context_tokens": context,
            "api_key": std::env::var("NOTE_TAKER_LLM_API_KEY").ok(),
        }),
        AiCommand::Remove { id } => json!({"command": "delete_connection", "id": id}),
        AiCommand::Default { id } => json!({"command": "update", "default_connection": id}),
        AiCommand::Test { id } => json!({"command": "test_connection", "id": id}),
        AiCommand::Auto { state } => json!({"command": "update", "auto_summarize": state == "on"}),
    };
    let answer = cli_service.llm_command(&request)?;
    if answer["ok"] != true {
        return Err(answer["error"]
            .as_str()
            .unwrap_or("the command failed")
            .to_owned());
    }
    Ok(match request["command"].as_str() {
        Some("save_connection") => format!(
            "Added {} ({}).",
            answer["connection"]["label"].as_str().unwrap_or_default(),
            answer["connection"]["id"].as_str().unwrap_or_default()
        ),
        Some("delete_connection") => String::from("Connection removed."),
        Some("test_connection") => format!(
            "The connection answered: {}",
            answer["reply"].as_str().unwrap_or_default()
        ),
        _ => format_ai_settings(&answer),
    })
}

/// Formats the AI settings as a table, keys left out.
fn format_ai_settings(settings: &serde_json::Value) -> String {
    let mut lines = vec![format!(
        "Automatic summaries: {}",
        if settings["auto_summarize"] == true {
            "on"
        } else {
            "off"
        }
    )];
    let connections = settings["connections"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if connections.is_empty() {
        lines.push(String::from("No AI connection yet; add one with `ai add`."));
    }
    for connection in connections {
        lines.push(format!(
            "{}{}  {}  {}  {}{}",
            if connection["is_default"] == true {
                "* "
            } else {
                "  "
            },
            connection["id"].as_str().unwrap_or_default(),
            connection["label"].as_str().unwrap_or_default(),
            connection["model"].as_str().unwrap_or_default(),
            connection["base_url"].as_str().unwrap_or_default(),
            if connection["has_key"] == true {
                "  (key saved)"
            } else {
                ""
            },
        ));
    }
    lines.join("\n")
}

/// Shows or changes the speech retention window (#93).
///
/// Inputs: The new window in days, or `None` to show it; the service holding it.
/// Outputs: The window or a confirmation; or why the window was refused, in which case nothing
/// is saved. Side effects: Saves the window when one is given.
fn keep_speech(days: Option<u16>, cli_service: &mut impl CliService) -> Result<String, String> {
    use crate::application::combined_meetings::{
        MAX_KEEP_SPEECH_DAYS, MIN_KEEP_SPEECH_DAYS, keep_for_days,
    };
    let Some(days) = days else {
        let current = cli_service.keep_speech_days()?;
        return Ok(format!(
            "Speech waits up to {current} days to be combined with another device's recording."
        ));
    };
    if keep_for_days(days).is_none() {
        return Err(format!(
            "choose between {MIN_KEEP_SPEECH_DAYS} and {MAX_KEEP_SPEECH_DAYS} days"
        ));
    }
    cli_service.save_keep_speech_days(days)?;
    Ok(format!(
        "Speech now waits up to {days} days to be combined; older speech is deleted at the next round."
    ))
}

/// Shows or switches keeping online-meeting participants' voices (#103).
///
/// Inputs: `on`/`off`, or `None` to show the choice; the service holding it.
/// Outputs: The choice or a confirmation. Side effects: Saves the choice when one is given.
fn meeting_voices(
    choice: Option<OnOff>,
    cli_service: &mut impl CliService,
) -> Result<String, String> {
    let Some(choice) = choice else {
        return Ok(if cli_service.keeps_meeting_voices()? {
            String::from(
                "Meeting participants' voices are kept in the voice gallery after enough speech of one voice.",
            )
        } else {
            String::from("Meeting participants' voices are not kept in the voice gallery.")
        });
    };
    let keep = choice == OnOff::On;
    cli_service.save_keep_meeting_voices(keep)?;
    Ok(if keep {
        String::from(
            "Meeting participants' voices will be kept in the voice gallery; meeting rooms never are.",
        )
    } else {
        String::from(
            "Meeting participants' voices will no longer be kept; kept voices stay until removed.",
        )
    })
}

/// Formats meeting rows as a stable terminal table.
fn format_meeting_table(meetings: Vec<MeetingListRow>) -> String {
    std::iter::once(String::from("ID | Title | Date | Duration | Devices"))
        .chain(meetings.into_iter().map(|meeting| {
            format!(
                "{} | {} | {} | {} | {}",
                meeting.meeting_id,
                meeting.title,
                meeting.created_at,
                meeting.duration,
                meeting.device_count
            )
        }))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats paired-device rows, or a sentence when there are none.
fn format_device_table(devices: Vec<PairedDeviceRow>) -> String {
    if devices.is_empty() {
        return String::from("No paired devices.");
    }
    std::iter::once(String::from("Peer ID | Name | Paired"))
        .chain(devices.into_iter().map(|device| {
            format!(
                "{} | {} | {}",
                device.peer_id, device.device_name, device.paired_at
            )
        }))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Formats a pairing time (Unix milliseconds) as `YYYY-MM-DD HH:MM UTC`, or `unknown` when it
/// is out of range.
fn format_paired_at(paired_at_ms: u64) -> String {
    i64::try_from(paired_at_ms)
        .ok()
        .and_then(DateTime::from_timestamp_millis)
        .map_or_else(
            || String::from("unknown"),
            |paired_at| paired_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        )
}

/// Finds the one paired device `device` names: an exact peer ID first, otherwise a name
/// compared without regard to case.
///
/// Inputs: The allowlist and the selector typed by the owner.
/// Outputs: The device, or a readable error when none or several match.
/// Side effects: None.
fn select_paired_device(peers: Vec<TrustedPeer>, device: &str) -> Result<TrustedPeer, String> {
    if let Some(peer) = peers.iter().find(|peer| peer.peer_id().as_str() == device) {
        return Ok(peer.clone());
    }
    let wanted_name = device.to_lowercase();
    let mut named: Vec<TrustedPeer> = peers
        .into_iter()
        .filter(|peer| peer.device_name().as_str().to_lowercase() == wanted_name)
        .collect();
    match named.len() {
        0 => Err(format!(
            "No paired device has the peer ID or name \"{device}\"."
        )),
        1 => named
            .pop()
            .ok_or_else(|| String::from("the paired device disappeared")),
        count => Err(format!(
            "{count} paired devices are named \"{device}\"; use the peer ID from `devices list`."
        )),
    }
}

/// Formats the `meetings view` language line, such as `Transcription languages: it, en\n`.
///
/// Inputs: The meeting's metadata. Outputs: The line with its newline, or an empty string when
/// no language was recorded. Side effects: None.
fn format_transcription_languages(meeting_metadata: &MeetingMetadata) -> String {
    let codes: Vec<&str> = meeting_metadata
        .transcription_languages()
        .iter()
        .map(|language| language.as_str())
        .collect();
    match codes.len() {
        0 => String::new(),
        1 => format!("Transcription language: {}\n", codes[0]),
        _ => format!("Transcription languages: {}\n", codes.join(", ")),
    }
}

/// Formats a UTC creation time for terminal output.
fn format_created_at(meeting_metadata: &MeetingMetadata) -> String {
    meeting_metadata
        .created_at()
        .format("%Y-%m-%d %H:%M UTC")
        .to_string()
}

/// Formats milliseconds as `M:SS`, or `H:MM:SS` from one hour, truncating partial seconds.
fn format_duration(duration_ms: u64) -> String {
    let total_seconds = duration_ms / 1_000;
    let (hours, minutes, seconds) = (
        total_seconds / 3_600,
        total_seconds / 60 % 60,
        total_seconds % 60,
    );
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
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

/// Adapts the recording control, repository, and voice-manager services for the CLI.
pub struct CoreCliService<RecordingPort, RepositoryPort, VectorDatabasePort> {
    recording_control: RecordingPort,
    /// Where `log-level` writes the daemon's requested verbosity; `None` disables the command.
    log_level_control: Option<LogLevelControlFile>,
    /// The allowlist of paired devices on the shared database; `None` disables `devices`.
    trusted_peer_store: Option<Box<dyn TrustedPeerStore>>,
    repository: RepositoryPort,
    voice_identity_manager: VoiceIdentityManager<VectorDatabasePort>,
    export_writer: ExportWriter,
    /// Checks for a recording and removes files on deletion; `None` skips both.
    deletion_hooks: Option<Box<dyn MeetingDeletionHooks>>,
    /// The transcription settings file holding the glossary; `None` disables `glossary`.
    settings_file: Option<std::path::PathBuf>,
    /// AI connections and summaries (#95); `None` disables `summary` and `ai`.
    llm: Option<std::sync::Arc<crate::llm_commands::LlmServices>>,
    /// Project files, so `search` reads their text (#100); `None` finds files by name only.
    project_files: Option<crate::project_commands::ProjectFiles>,
    /// The notes folder beside the database (#98); `None` disables `notes-folder`.
    notes_folder: Option<crate::notes_folder_commands::NotesFolderKeeper>,
    /// This device and the folder of the speech it holds, for `meetings view`'s combining line
    /// (#88); `None` says nothing is held here.
    recording_device: Option<(
        DeviceId,
        crate::adapters::audio::retained_audio_store::RetainedSpeechIndex,
    )>,
}

impl<RecordingPort, RepositoryPort, VectorDatabasePort>
    CoreCliService<RecordingPort, RepositoryPort, VectorDatabasePort>
where
    VectorDatabasePort: VoiceVectorDB,
{
    /// Composes a CLI service from existing adapters and the voice identity application service.
    ///
    /// Inputs: A recording control, repository port, vector database port, and the export writer
    /// (exporters plus default export directory) composed by the binary.
    /// Outputs: A driving adapter service with no SQLite or business-policy implementation.
    /// Side effects: Takes ownership of supplied dependencies without calling them.
    pub fn new(
        recording_control: RecordingPort,
        repository: RepositoryPort,
        vector_database: VectorDatabasePort,
        export_writer: ExportWriter,
    ) -> Self {
        Self {
            recording_control,
            log_level_control: None,
            trusted_peer_store: None,
            repository,
            voice_identity_manager: VoiceIdentityManager::new(vector_database),
            export_writer,
            deletion_hooks: None,
            settings_file: None,
            llm: None,
            project_files: None,
            notes_folder: None,
            recording_device: None,
        }
    }

    /// Enables `summary` and `ai` with the AI connection services. Side effects: None.
    pub fn with_llm_services(
        mut self,
        llm: std::sync::Arc<crate::llm_commands::LlmServices>,
    ) -> Self {
        self.llm = Some(llm);
        self
    }

    /// Attaches what deleting a meeting needs from outside the database.
    ///
    /// Side effects: None; without it deletions skip the recording check and leave files.
    pub fn with_deletion_hooks(mut self, deletion_hooks: Box<dyn MeetingDeletionHooks>) -> Self {
        self.deletion_hooks = Some(deletion_hooks);
        self
    }

    /// Names this device and the folder of the speech it holds, which `meetings view` reads to
    /// say how far combining a meeting's recordings got (#88).
    ///
    /// Side effects: None.
    pub fn with_recording_device(
        mut self,
        this_device: DeviceId,
        retained_audio_directory: std::path::PathBuf,
    ) -> Self {
        self.recording_device = Some((
            this_device,
            crate::adapters::audio::retained_audio_store::RetainedSpeechIndex::new(
                retained_audio_directory,
            ),
        ));
        self
    }

    /// Describes how far combining a listed meeting's recordings got, for `meetings view`.
    ///
    /// Outputs: One line ending in a newline, or nothing for a meeting of one device whose speech
    /// is gone. Side effects: Reads the meetings index, the retained speech folder's metadata,
    /// and the paired devices' names.
    fn combining_line(&self, meeting_id: &MeetingId) -> Result<String, String>
    where
        RepositoryPort: NoteRepository,
    {
        let recordings = self
            .repository
            .meeting_recordings()
            .map_err(|error| error.to_string())?;
        let now = std::time::SystemTime::now();
        let unknown_device = DeviceId::new("");
        let (this_device, index) = match &self.recording_device {
            Some((device, index)) => (device, Some(index)),
            None => (&unknown_device, None),
        };
        let held = |meeting: &MeetingId| index.and_then(|index| index.held_speech(meeting, now));
        let status = crate::application::combined_meetings::combining_status(
            meeting_id,
            &recordings,
            &held,
            this_device,
            chrono::Utc::now(),
            crate::transcription_settings::keep_for_in(self.settings_file.as_deref()),
        );
        let paired = self
            .trusted_peer_store
            .as_deref()
            .and_then(|store| store.trusted_peers().ok())
            .unwrap_or_default();
        let name = |device: &DeviceId| {
            if device == this_device {
                return String::from("this device");
            }
            paired
                .iter()
                .find(|peer| peer.storage_device_id() == Some(device))
                .map_or_else(
                    || device.as_str().to_owned(),
                    |peer| peer.device_name().as_str().to_owned(),
                )
        };
        Ok(describe_combining(&status, &name))
    }

    /// Enables `notes-folder` with its settings beside the database (#98).
    ///
    /// Inputs: The database path and the home folder, for the suggested `~/NoteTaker`.
    /// Outputs: The service. Side effects: None.
    pub fn with_notes_folder(
        mut self,
        database_path: &std::path::Path,
        home: Option<PathBuf>,
    ) -> Self {
        self.notes_folder = Some(
            crate::notes_folder_commands::NotesFolderKeeper::for_database(database_path, home),
        );
        self
    }

    /// Lets `search` read project files' text in `attachment_directory` (#100).
    ///
    /// Inputs: The attachment folder, this device, and the sealer holding the database key
    /// (#99). Outputs: The service. Side effects: None.
    pub fn with_project_files(
        mut self,
        attachment_directory: PathBuf,
        device_id: DeviceId,
        sealed_files: crate::adapters::storage::sealed_files::SealedFiles,
    ) -> Self {
        self.project_files = Some(crate::project_commands::ProjectFiles::new(
            attachment_directory,
            device_id,
            sealed_files,
        ));
        self
    }

    /// Enables `glossary` with the transcription settings file beside the database, which every
    /// app and the daemon on that database read.
    ///
    /// Inputs: The settings file. Outputs: The service. Side effects: None.
    pub fn with_settings_file(mut self, settings_file: std::path::PathBuf) -> Self {
        self.settings_file = Some(settings_file);
        self
    }

    /// Borrows the settings file, or explains that the glossary is not available here.
    fn settings_file(&self) -> Result<&std::path::Path, String> {
        self.settings_file
            .as_deref()
            .ok_or_else(|| String::from("the glossary is not available here"))
    }

    /// Enables `log-level` by naming the control file a daemon on the same database polls.
    ///
    /// Inputs: The database's `LogLevelControlFile`. Outputs: The service. Side effects: None.
    pub fn with_log_level_control(mut self, log_level_control: LogLevelControlFile) -> Self {
        self.log_level_control = Some(log_level_control);
        self
    }

    /// Enables `devices` with the allowlist stored in the shared database.
    ///
    /// Inputs: The database's `TrustedPeerStore` (the same table a daemon's P2P task reads).
    /// Outputs: The service. Side effects: None.
    pub fn with_trusted_peer_store(
        mut self,
        trusted_peer_store: Box<dyn TrustedPeerStore>,
    ) -> Self {
        self.trusted_peer_store = Some(trusted_peer_store);
        self
    }

    /// Borrows the allowlist, or explains that device commands are not available.
    fn trusted_peer_store(&self) -> Result<&dyn TrustedPeerStore, String> {
        self.trusted_peer_store
            .as_deref()
            .ok_or_else(|| String::from("managing paired devices is not available here"))
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

impl<RecordingPort, RepositoryPort, VectorDatabasePort>
    CoreCliService<RecordingPort, RepositoryPort, VectorDatabasePort>
where
    RepositoryPort: NoteRepository,
{
    /// Works out which meetings are listed and which devices recorded each (#88).
    ///
    /// Outputs: The listing, or a storage error message. Side effects: Reads meeting metadata.
    fn meeting_listing(&self) -> Result<MeetingListing, String> {
        self.repository
            .meeting_recordings()
            .map(|recordings| MeetingListing::of(&recordings))
            .map_err(|error| error.to_string())
    }
}

impl<RecordingPort, RepositoryPort, VectorDatabasePort> CliService
    for CoreCliService<RecordingPort, RepositoryPort, VectorDatabasePort>
where
    RecordingPort: RecordingControl,
    RepositoryPort: NoteRepository
        + AttachmentRepository
        + MetricsRepository
        + crate::ports::projects::ProjectRepository
        + crate::ports::chat::ChatHistoryRepository,
    VectorDatabasePort: VoiceVectorDB,
{
    fn summarize_meeting(
        &mut self,
        meeting_id: &str,
        connection: Option<&str>,
    ) -> Result<String, String> {
        let llm = self
            .llm
            .clone()
            .ok_or_else(|| String::from("summaries are not available here"))?;
        let meeting_id = MeetingId::new(meeting_id);
        if self
            .repository
            .load_meeting_metadata(&meeting_id)
            .map_err(|error| error.to_string())?
            .is_none()
        {
            return Err(String::from("there is no such meeting"));
        }
        let repository = &self.repository;
        let voices = self.voice_identity_manager.vector_database();
        let answer = crate::llm_commands::summarize_with(
            &llm,
            connection,
            |glossary, device_name| {
                crate::meeting_summaries::prepare_summary(
                    repository,
                    voices,
                    &meeting_id,
                    glossary,
                    device_name,
                )
            },
            |summary| crate::meeting_summaries::store_summary(repository, &meeting_id, summary),
        );
        if answer["ok"] != true {
            return Err(answer["error"]
                .as_str()
                .unwrap_or("the summary failed")
                .to_owned());
        }
        Ok(answer["summary"]["text"]
            .as_str()
            .unwrap_or_default()
            .to_owned())
    }
    fn search(
        &self,
        request: &crate::search_commands::SearchRequest,
    ) -> Result<serde_json::Value, String> {
        let names =
            crate::search_commands::speaker_names(self.voice_identity_manager.vector_database())
                .map_err(|error| error.to_string())?;
        let sources =
            crate::search_commands::HostSearchSources::new(names, self.project_files.as_ref());
        Ok(crate::search_commands::run_search(
            &self.repository,
            &sources,
            request,
        ))
    }
    fn chat(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        use crate::notes_chat_commands::{
            ChatCommand, ask_with, gather_for_ask, parse_chat_request, run_chat_store_command,
            store_messages,
        };
        let command = match parse_chat_request(&request.to_string()) {
            Ok(command) => command,
            Err(error) => return Ok(error.to_json()),
        };
        let ChatCommand::Ask {
            scope,
            question,
            connection_id,
        } = command
        else {
            return Ok(run_chat_store_command(&self.repository, &command));
        };
        let llm = self
            .llm
            .clone()
            .ok_or_else(|| String::from("AI connections are not available here"))?;
        let repository = &self.repository;
        let voices = self.voice_identity_manager.vector_database();
        let files = self.project_files.as_ref();
        Ok(ask_with(
            &llm,
            connection_id.as_deref(),
            &question,
            || gather_for_ask(repository, voices, files, &scope, &question),
            |messages| store_messages(repository, &scope, messages),
        ))
    }
    fn notes_folder(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        let (Some(keeper), Some(files)) = (&self.notes_folder, &self.project_files) else {
            return Err(String::from("the notes folder is not available here"));
        };
        let names =
            crate::search_commands::speaker_names(self.voice_identity_manager.vector_database())
                .map_err(|error| error.to_string())?;
        let repository = &self.repository;
        Ok(keeper.command(&request.to_string(), &mut |root, state| {
            crate::notes_folder::run_round(
                repository,
                files,
                &names,
                root,
                state,
                chrono::Utc::now(),
            )
        }))
    }
    fn meeting_summary(&self, meeting_id: &str) -> Result<Option<String>, String> {
        crate::meeting_summaries::load_summary(&self.repository, &MeetingId::new(meeting_id))
            .map(|summary| summary.map(|summary| summary.text))
            .map_err(|error| error.to_string())
    }
    fn llm_command(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        let llm = self
            .llm
            .as_deref()
            .ok_or_else(|| String::from("AI connections are not available here"))?;
        serde_json::from_str(&crate::llm_commands::run_llm_command(
            llm,
            &request.to_string(),
        ))
        .map_err(|error| error.to_string())
    }
    fn start_recording(
        &mut self,
        meeting_id: Option<&str>,
        title: Option<&str>,
        input: Option<&Path>,
        language: Option<&str>,
    ) -> Result<String, String> {
        let meeting_id = match meeting_id.map(str::trim) {
            Some("") => return Err(String::from("--meeting-id must not be blank")),
            Some(meeting_id) => Some(MeetingId::new(meeting_id)),
            None => None,
        };
        let title = title
            .map(MeetingTitle::parse)
            .transpose()
            .map_err(|error| format!("--title: {error}"))?;
        let language = language
            .map(TranscriptionLanguage::parse)
            .transpose()
            .map_err(|_| {
                String::from("--language: use auto or a Whisper language code such as it or en")
            })?;
        self.recording_control
            .record(meeting_id, title, input, language)
    }
    fn stop_recording(&mut self) -> Result<String, String> {
        self.recording_control.request_stop()
    }
    fn recording_status(&self) -> Result<String, String> {
        self.recording_control.status()
    }
    fn list_meetings(&self) -> Result<Vec<MeetingListRow>, String> {
        let meeting_ids = self
            .repository
            .list_meeting_ids()
            .map_err(|error| error.to_string())?;
        let listing = self.meeting_listing()?;
        let mut meeting_rows = Vec::with_capacity(meeting_ids.len());
        for meeting_id in meeting_ids {
            if !listing.is_listed(&meeting_id) {
                continue;
            }
            let Some(meeting_metadata) = self
                .repository
                .load_meeting_metadata(&meeting_id)
                .map_err(|error| error.to_string())?
            else {
                continue;
            };
            let duration_ms = self
                .repository
                .load_transcript_segments(&meeting_id)
                .map_err(|error| error.to_string())?
                .iter()
                .map(|segment| segment.end_time_ms())
                .max()
                .unwrap_or_default();
            meeting_rows.push(MeetingListRow {
                meeting_id: String::from(meeting_id.as_str()),
                title: meeting_metadata.display_title(),
                created_at: format_created_at(&meeting_metadata),
                duration: format_duration(duration_ms),
                device_count: listing.recorded_on(&meeting_id).len().max(1),
            });
        }
        Ok(meeting_rows)
    }
    fn view_meeting(&self, meeting_id: &str) -> Result<String, String> {
        let typed_meeting_id = MeetingId::new(meeting_id);
        let meeting_metadata = self
            .repository
            .load_meeting_metadata(&typed_meeting_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("meeting {meeting_id} does not exist"))?;
        let display_title = meeting_metadata.display_title();
        let title_underline = "=".repeat(display_title.chars().count());
        let created_at = format_created_at(&meeting_metadata);
        let language_line = format_transcription_languages(&meeting_metadata);
        let transcript = crate::application::meeting_transcript::readable_transcript(
            &self.repository,
            &typed_meeting_id,
        )
        .map_err(|error| error.to_string())?
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
        let listing = self.meeting_listing()?;
        let folded = listing.folded_into(&typed_meeting_id);
        let devices_line = if folded.is_empty() {
            String::new()
        } else {
            format!(
                "Recorded on {} devices (also {}); `meetings split {meeting_id}` lists them apart\n",
                listing.recorded_on(&typed_meeting_id).len(),
                folded
                    .iter()
                    .map(MeetingId::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let combining_line = self.combining_line(&typed_meeting_id)?;
        Ok(format!(
            "{display_title}\n{title_underline}\nMeeting: {meeting_id}\nCreated: {created_at}\n{devices_line}{combining_line}{language_line}Transcript:\n{transcript}\nAttachments:\n{attachment_lines}"
        ))
    }
    /// Renames through the shared use case and prints `Renamed meeting <id>: "<old>" -> "<new>"`,
    /// where `<old>` is the dated fallback for a previously untitled meeting.
    ///
    /// Inputs: The meeting ID and the unvalidated new title.
    /// Outputs: The confirmation line, or `meeting <id> does not exist`, `invalid title: <reason>`,
    /// or a storage error message.
    /// Side effects: Replaces the title in SQLite; an invalid title writes nothing.
    fn rename_meeting(&mut self, meeting_id: &str, new_title: &str) -> Result<String, String> {
        rename_meeting(&self.repository, &MeetingId::new(meeting_id), new_title)
            .map(|rename| {
                format!(
                    "Renamed meeting {}: \"{}\" -> \"{}\"",
                    rename.meeting_id().as_str(),
                    rename.previous_title(),
                    rename.new_title().as_str()
                )
            })
            .map_err(|error| error.to_string())
    }
    fn delete_meeting(&mut self, meeting_id: &str) -> Result<String, String> {
        deletion::delete_meeting(
            &self.repository,
            self.deletion_hooks.as_deref(),
            &MeetingId::new(meeting_id),
        )
        .map(|()| format!("Deleted meeting {meeting_id}"))
        .map_err(|error| error.to_string())
    }
    /// Splits through the shared use case and prints which recordings are listed again.
    ///
    /// Outputs: `Split meeting <id>: <ids> is/are listed on its/their own again`, or a sentence
    /// saying there was nothing to split, or a storage error message.
    /// Side effects: Writes each split recording's document, which syncs to paired devices.
    fn split_meeting(&mut self, meeting_id: &str) -> Result<String, String> {
        let split = split_recordings(&self.repository, &MeetingId::new(meeting_id))
            .map_err(|error| error.to_string())?;
        Ok(match split.as_slice() {
            [] => format!("Meeting {meeting_id} combines no other recording; nothing to split"),
            [one] => format!(
                "Split meeting {meeting_id}: {} is listed on its own again",
                one.as_str()
            ),
            several => format!(
                "Split meeting {meeting_id}: {} are listed on their own again",
                several
                    .iter()
                    .map(MeetingId::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })
    }
    fn delete_unknown_voice(&mut self, cluster_id: Uuid) -> Result<String, String> {
        deletion::delete_unknown_voice(&self.repository, self.deletion_hooks.as_deref(), cluster_id)
            .map(|()| format!("Deleted unknown voice {cluster_id}"))
            .map_err(|error| error.to_string())
    }
    fn delete_saved_person(&mut self, identity_id: Uuid) -> Result<String, String> {
        self.voice_identity_manager
            .vector_database()
            .delete_identity(identity_id)
            .map(|()| format!("Deleted person {identity_id}"))
            .map_err(|error| error.to_string())
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
    fn request_log_level(&mut self, filter_directive: &str) -> Result<String, String> {
        let log_level_control = self
            .log_level_control
            .as_ref()
            .ok_or_else(|| String::from("changing the daemon log level is not available here"))?;
        log_level_control
            .request(filter_directive)
            .map_err(|error| format!("{error:#}"))?;
        Ok(format!(
            "Requested log level \"{}\"; a running daemon on this database applies it within {} seconds.",
            filter_directive.trim(),
            crate::adapters::observability::LOG_LEVEL_POLL_INTERVAL.as_secs()
        ))
    }
    fn list_paired_devices(&self) -> Result<Vec<PairedDeviceRow>, String> {
        let peers = self
            .trusted_peer_store()?
            .trusted_peers()
            .map_err(|error| error.to_string())?;
        Ok(peers
            .into_iter()
            .map(|peer| PairedDeviceRow {
                peer_id: peer.peer_id().as_str().to_owned(),
                device_name: peer.device_name().as_str().to_owned(),
                paired_at: format_paired_at(peer.paired_at_ms()),
            })
            .collect())
    }
    fn glossary(&self) -> Result<Vec<String>, String> {
        let settings_file = self.settings_file()?;
        Ok(crate::transcription_settings::load_settings(settings_file).glossary)
    }
    fn save_glossary(&mut self, terms: &[String]) -> Result<(), String> {
        let settings_file = self.settings_file()?;
        let glossary = TranscriptionGlossary::parse(terms.iter().map(String::as_str))
            .map_err(|error| error.to_string())?;
        crate::transcription_settings::save_glossary(settings_file, &glossary)
            .map_err(|error| format!("the glossary could not be saved: {error}"))
    }
    fn keep_speech_days(&self) -> Result<u16, String> {
        let settings_file = self.settings_file()?;
        Ok(crate::transcription_settings::keep_speech_days(
            crate::transcription_settings::keep_for_in(Some(settings_file)),
        ))
    }
    fn save_keep_speech_days(&mut self, days: u16) -> Result<(), String> {
        let settings_file = self.settings_file()?;
        crate::transcription_settings::save_keep_speech_days(settings_file, days)
            .map_err(|error| format!("the window could not be saved: {error}"))
    }
    fn keeps_meeting_voices(&self) -> Result<bool, String> {
        Ok(crate::transcription_settings::keeps_meeting_voices(
            self.settings_file()?,
        ))
    }
    fn save_keep_meeting_voices(&mut self, keep: bool) -> Result<(), String> {
        let settings_file = self.settings_file()?;
        crate::transcription_settings::save_keep_meeting_voices(settings_file, keep)
            .map_err(|error| format!("the choice could not be saved: {error}"))
    }
    fn revoke_device(&mut self, device: &str) -> Result<String, String> {
        let trusted_peer_store = self.trusted_peer_store()?;
        let peers = trusted_peer_store
            .trusted_peers()
            .map_err(|error| error.to_string())?;
        let peer = select_paired_device(peers, device.trim())?;
        trusted_peer_store
            .revoke_peer(peer.peer_id())
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "Removed {} ({}). A running daemon on this database stops syncing with it within a second.",
            peer.device_name().as_str(),
            peer.peer_id().as_str()
        ))
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

/// Words a meeting's combining status as one line of `meetings view` (#88).
///
/// Inputs: The status and how to name a device. Outputs: The line ending in a newline, or
/// nothing for a meeting of one device whose speech is gone. Side effects: None.
fn describe_combining(
    status: &crate::application::combined_meetings::CombiningStatus,
    name: &dyn Fn(&DeviceId) -> String,
) -> String {
    use crate::application::combined_meetings::CombiningState;
    let names = |devices: &[DeviceId]| devices.iter().map(name).collect::<Vec<_>>().join(", ");
    let until = status
        .kept_until
        .map(|until| format!("; speech kept here until {}", until.format("%Y-%m-%d")))
        .unwrap_or_default();
    let combine_on = status.combine_on.as_ref().map(name).unwrap_or_default();
    let line = match status.state {
        CombiningState::OneDevice if status.kept_until.is_none() => return String::new(),
        CombiningState::OneDevice => {
            format!("one device so far, a recording on another device can still join{until}")
        }
        CombiningState::Waiting if status.waiting_for.is_empty() => {
            format!("waiting for {combine_on} to combine the recordings")
        }
        CombiningState::Waiting => format!(
            "waiting for speech from {} to reach {combine_on}{until}",
            names(&status.waiting_for)
        ),
        CombiningState::ReadyToCombine => {
            format!("every recording is on {combine_on}; combining when the device is free")
        }
        CombiningState::Sending => {
            format!("this device's speech still has to reach {combine_on}{until}")
        }
        CombiningState::Combined => String::from("transcript combined from every device"),
        CombiningState::NotCombined => format!(
            "not combined: speech from {} did not arrive in time",
            names(&status.waiting_for)
        ),
    };
    format!("Combining: {line}\n")
}
