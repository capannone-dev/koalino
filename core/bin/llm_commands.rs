//! Runs the language model commands (#95) for the app and the web API, so both behave alike.
//!
//! The app sends one JSON object tagged by `command` through `note_taker_llm_command`; the web
//! routes in `server/llm_routes.rs` call the same functions. Every answer is `{"ok":true,...}`
//! or `{"ok":false,"error_code","error"}`; an `llm_rejected` failure adds `error_detail`, the AI
//! server's own reason (#104), for the owner to read. It is never logged.
//!
//! | `command` | Fields | Answer |
//! | --- | --- | --- |
//! | `settings` | | `connections`, `default_connection`, `template`, `default_template`, `custom_template`, `auto_summarize`, `placeholders` |
//! | `save_connection` | `id`? (absent creates), `label`, `kind`, `base_url` (not for `claude_code`/`codex`), `model` (optional for those), `context_tokens`, `api_key`? (ignored for those), `clear_key`? | `connection` |
//! | `delete_connection` | `id` | `deleted: true` (its key goes too) |
//! | `update` | `default_connection`?, `template`? (null resets), `auto_summarize`? | the settings |
//! | `test_connection` | `id` | `reply`: the model's answer to a fixed one-line request, which holds no meeting content |
//!
//! A connection is shown as `{id,label,kind,base_url,model,context_tokens,plain_http,has_key,tool_installed,
//! is_default}`; keys are write-only and never in an answer or a log. Meeting summaries go
//! through `summarize_with`, which lets the caller hold its repository only while reading and
//! writing, not while the model works.
//!
//! Error codes: `invalid_request`, `invalid_connection`, `invalid_template`,
//! `connection_not_found`, `no_connection`, `missing_key`, `keychain_failed`, `save_failed`,
//! `nothing_to_summarize`, `too_long`, `llm_unreachable`, `llm_unauthorized`, `llm_rate_limited`,
//! `llm_rejected`, `llm_failed`, `llm_refused`, `llm_invalid_response`, `llm_tool_missing`,
//! `llm_tool_failed` and `storage_failed`. `llm_tool_failed` also carries `error_detail`, what
//! Claude Code or Codex said (#105).

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::{
    llm_settings::{LlmKeyring, SavedLlmSettings, load_llm_settings, save_llm_settings},
    meeting_summaries::{PreparedSummary, run_summary},
    transcription_settings::{load_settings, saved_glossary},
};
use crate::{
    adapters::ai::llm::{cli_agent::find_tool, engine_for},
    application::meeting_summary::SummaryError,
    domain::{
        errors::LlmError,
        llm::{
            DEFAULT_SUMMARY_TEMPLATE, LlmConnection, LlmKind, LlmRequest, LlmRole, LlmTurn,
            MeetingSummary, PLACEHOLDERS,
        },
    },
    ports::ai::LlmEngine,
};

/// Builds the engine for a connection; tests replace it with a fake model.
pub type EngineFactory = Arc<
    dyn Fn(&LlmConnection, Option<Zeroizing<String>>) -> Result<Box<dyn LlmEngine>, LlmError>
        + Send
        + Sync,
>;

/// What the commands need: the settings file, the keys, the model factory and this device.
pub struct LlmServices {
    settings_file: PathBuf,
    keys: Arc<dyn LlmKeyring>,
    engines: EngineFactory,
    device_name: String,
    transcription_settings: Option<PathBuf>,
    /// Serializes changes to the settings file within this process.
    writing: Mutex<()>,
}

impl LlmServices {
    /// Creates the services with the real model adapters.
    ///
    /// Inputs: `<db>.llm-settings.json`, the keyring and this device's name. Outputs: The
    /// services. Side effects: None.
    pub fn new(settings_file: PathBuf, keys: Arc<dyn LlmKeyring>, device_name: &str) -> Self {
        Self {
            settings_file,
            keys,
            engines: Arc::new(engine_for),
            device_name: device_name.to_owned(),
            transcription_settings: None,
            writing: Mutex::new(()),
        }
    }

    /// Builds the services of a daemon or CLI on a database: settings beside it, keys in the
    /// platform keychain, the glossary from the transcription settings, and the device name from
    /// `NOTE_TAKER_DEVICE_NAME`.
    ///
    /// Side effects: Reads the environment.
    pub fn for_database(database_path: &std::path::Path) -> Self {
        let key_directory = std::env::var("NOTE_TAKER_ENCRYPTION_KEY_DIR").map_or_else(
            |_| {
                database_path
                    .parent()
                    .map_or_else(PathBuf::new, std::path::Path::to_path_buf)
                    .join("keys")
            },
            PathBuf::from,
        );
        let device_name = std::env::var("NOTE_TAKER_DEVICE_NAME")
            .unwrap_or_else(|_| String::from("this computer"));
        Self::new(
            super::llm_settings::llm_settings_file_for(database_path),
            Arc::new(super::llm_settings::PlatformLlmKeyring::new(
                key_directory,
                None,
            )),
            &device_name,
        )
        .with_transcription_settings(super::transcription_settings::settings_file_for(
            database_path,
        ))
    }

    /// Reads the glossary from the transcription settings file. Side effects: None.
    pub fn with_transcription_settings(mut self, settings_file: PathBuf) -> Self {
        self.transcription_settings = Some(settings_file);
        self
    }

    /// Replaces the model factory, as tests do. Side effects: None.
    pub fn with_engine_factory(mut self, engines: EngineFactory) -> Self {
        self.engines = engines;
        self
    }

    /// Reads the current settings. Side effects: Reads the file.
    pub fn settings(&self) -> SavedLlmSettings {
        load_llm_settings(&self.settings_file)
    }

    /// Builds the engine for a connection with its saved key.
    pub(crate) fn engine(
        &self,
        connection: &LlmConnection,
    ) -> Result<Box<dyn LlmEngine>, CommandError> {
        let key = self
            .keys
            .load(connection.id())
            .map_err(|_| CommandError::new("keychain_failed", "the API key could not be read"))?;
        (self.engines)(connection, key).map_err(CommandError::from)
    }

    /// Changes the settings and saves them, one change at a time.
    fn change(
        &self,
        edit: impl FnOnce(&mut SavedLlmSettings) -> Result<(), CommandError>,
    ) -> Result<SavedLlmSettings, CommandError> {
        let _writing = self
            .writing
            .lock()
            .map_err(|_| CommandError::save_failed())?;
        let mut settings = self.settings();
        edit(&mut settings)?;
        save_llm_settings(&self.settings_file, &settings)
            .map_err(|_| CommandError::save_failed())?;
        Ok(settings)
    }

    /// Reads the glossary terms. Side effects: Reads the transcription settings file.
    fn glossary(&self) -> Vec<String> {
        self.transcription_settings
            .as_deref()
            .and_then(|file| saved_glossary(&load_settings(file)))
            .map(|glossary| glossary.terms().to_vec())
            .unwrap_or_default()
    }
}

/// A command failure with its stable code.
#[derive(Debug)]
pub struct CommandError {
    /// The stable error code.
    pub code: &'static str,
    /// A message for people, free of meeting content and keys.
    pub message: String,
    /// The AI server's own reason for a rejection, shown to the owner as `error_detail` and never
    /// logged: one short line that could, from a local server, echo meeting content.
    pub detail: Option<String>,
}

impl CommandError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    fn save_failed() -> Self {
        Self::new("save_failed", "the settings could not be saved")
    }

    /// Words the failure as `{"ok":false,"error_code","error"}`, plus `error_detail` when the AI
    /// server gave a reason. Side effects: None.
    pub fn to_json(&self) -> Value {
        let mut answer = json!({"ok": false, "error_code": self.code, "error": self.message});
        if let Some(detail) = &self.detail {
            answer["error_detail"] = json!(detail);
        }
        answer
    }
}

impl From<LlmError> for CommandError {
    fn from(error: LlmError) -> Self {
        let detail = match &error {
            LlmError::Rejected {
                provider_reason, ..
            } => provider_reason.clone(),
            LlmError::ToolFailed { reason } => reason.clone(),
            _ => None,
        };
        let code = match error {
            LlmError::Unreachable => "llm_unreachable",
            LlmError::Unauthorized => "llm_unauthorized",
            LlmError::RateLimited => "llm_rate_limited",
            LlmError::Rejected { .. } | LlmError::InvalidRequest { .. } => "llm_rejected",
            LlmError::ServerFailed { .. } => "llm_failed",
            LlmError::Refused => "llm_refused",
            LlmError::InvalidResponse => "llm_invalid_response",
            LlmError::MissingKey => "missing_key",
            LlmError::ToolMissing { .. } => "llm_tool_missing",
            LlmError::ToolFailed { .. } => "llm_tool_failed",
        };
        Self {
            detail,
            ..Self::new(code, error.to_string())
        }
    }
}

impl From<SummaryError> for CommandError {
    fn from(error: SummaryError) -> Self {
        match error {
            SummaryError::NothingToSummarize => {
                Self::new("nothing_to_summarize", error.to_string())
            }
            SummaryError::TooLong => Self::new("too_long", error.to_string()),
            SummaryError::Llm(error) => error.into(),
            SummaryError::Storage(_) => Self::new("storage_failed", error.to_string()),
        }
    }
}

/// Runs one JSON command.
///
/// Inputs: The services and the request. Outputs: The JSON answer. Side effects: As the command
/// says; never logs keys or meeting content.
pub fn run_llm_command(services: &LlmServices, request_json: &str) -> String {
    let answer = serde_json::from_str::<Value>(request_json)
        .map_err(|_| CommandError::new("invalid_request", "the request must be a JSON object"))
        .and_then(|request| dispatch(services, &request));
    match answer {
        Ok(mut answer) => {
            answer["ok"] = json!(true);
            answer.to_string()
        }
        Err(error) => error.to_json().to_string(),
    }
}

/// Picks the command.
fn dispatch(services: &LlmServices, request: &Value) -> Result<Value, CommandError> {
    match request.get("command").and_then(Value::as_str) {
        Some("settings") => Ok(settings_json(services, &services.settings())),
        Some("save_connection") => save_connection(services, request),
        Some("delete_connection") => delete_connection(services, text(request, "id")?),
        Some("update") => update(services, request),
        Some("test_connection") => test_connection(services, text(request, "id")?),
        _ => Err(CommandError::new("invalid_request", "unknown command")),
    }
}

/// Reads a required text field.
fn text<'request>(request: &'request Value, field: &str) -> Result<&'request str, CommandError> {
    request
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CommandError::new("invalid_request", format!("`{field}` is required")))
}

/// Describes the settings, keys left out.
pub fn settings_json(services: &LlmServices, settings: &SavedLlmSettings) -> Value {
    let default_id = settings.default_connection().map(LlmConnection::id);
    json!({
        "connections": settings
            .connections()
            .iter()
            .map(|connection| connection_json(services, connection, default_id))
            .collect::<Vec<_>>(),
        "default_connection": default_id,
        "template": settings.template().as_str(),
        "default_template": DEFAULT_SUMMARY_TEMPLATE,
        "custom_template": settings.has_custom_template(),
        "auto_summarize": settings.auto_summarize(),
        "placeholders": PLACEHOLDERS,
    })
}

/// Describes one connection; `has_key` says only whether a key is saved.
fn connection_json(
    services: &LlmServices,
    connection: &LlmConnection,
    default_id: Option<&str>,
) -> Value {
    let has_key = services.keys.load(connection.id()).ok().flatten().is_some();
    json!({
        "id": connection.id(),
        "label": connection.label(),
        "kind": connection.kind(),
        "base_url": connection.base_url(),
        "model": connection.model(),
        "context_tokens": connection.context_tokens(),
        "plain_http": connection.is_plain_http(),
        "has_key": has_key,
        // For Claude Code and Codex: whether the tool is installed here (#105).
        "tool_installed": connection.kind().is_local_cli()
            && find_tool(connection.kind()).is_some(),
        "is_default": default_id == Some(connection.id()),
    })
}

/// Creates or edits a connection and saves or clears its key.
fn save_connection(services: &LlmServices, request: &Value) -> Result<Value, CommandError> {
    let kind: LlmKind = request
        .get("kind")
        .cloned()
        .and_then(|kind| serde_json::from_value(kind).ok())
        .ok_or_else(|| {
            CommandError::new(
                "invalid_request",
                "`kind` is open_ai_compatible, anthropic, claude_code or codex",
            )
        })?;
    let context_tokens = request
        .get("context_tokens")
        .and_then(Value::as_u64)
        .and_then(|tokens| u32::try_from(tokens).ok())
        .unwrap_or(0);
    // Claude Code and Codex (#105) have no address, and may leave the model to the tool.
    let (label, base_url, model) = if kind.is_local_cli() {
        (
            text(request, "label")?,
            "",
            request.get("model").and_then(Value::as_str).unwrap_or(""),
        )
    } else {
        (
            text(request, "label")?,
            text(request, "base_url")?,
            text(request, "model")?,
        )
    };
    let checked = match request.get("id").and_then(Value::as_str) {
        Some(id) => LlmConnection::with_id(id, label, kind, base_url, model, context_tokens),
        None => LlmConnection::new(label, kind, base_url, model, context_tokens),
    }
    .map_err(|error| CommandError::new("invalid_connection", error.to_string()))?;
    let api_key = request
        .get("api_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty() && !kind.is_local_cli());
    if let Some(key) = api_key
        && crate::transcription_settings::validated_key(key).is_none()
    {
        return Err(CommandError::new(
            "invalid_connection",
            "the API key holds spaces or is too long",
        ));
    }
    let settings = services.change(|settings| {
        settings.upsert_connection(checked.clone());
        Ok(())
    })?;
    let keychain_failed =
        |_| CommandError::new("keychain_failed", "the API key could not be saved");
    if let Some(key) = api_key {
        services
            .keys
            .save(checked.id(), key)
            .map_err(keychain_failed)?;
    } else if request.get("clear_key").and_then(Value::as_bool) == Some(true) {
        services
            .keys
            .remove(checked.id())
            .map_err(keychain_failed)?;
    }
    let default_id = settings.default_connection().map(LlmConnection::id);
    Ok(json!({"connection": connection_json(services, &checked, default_id)}))
}

/// Removes a connection and its key.
fn delete_connection(services: &LlmServices, id: &str) -> Result<Value, CommandError> {
    services.change(|settings| {
        if settings.remove_connection(id) {
            Ok(())
        } else {
            Err(CommandError::new(
                "connection_not_found",
                "there is no such connection",
            ))
        }
    })?;
    // The connection is gone either way; a key left behind is unreachable.
    if services.keys.remove(id).is_err() {
        tracing::warn!("the removed connection's API key could not be deleted");
    }
    Ok(json!({"deleted": true}))
}

/// Changes the default connection, the template and automatic summaries, each only when named.
fn update(services: &LlmServices, request: &Value) -> Result<Value, CommandError> {
    let settings = services.change(|settings| {
        if let Some(default) = request.get("default_connection") {
            settings
                .set_default(default.as_str())
                .map_err(|error| CommandError::new("connection_not_found", error.to_string()))?;
        }
        if let Some(template) = request.get("template") {
            settings
                .set_template(template.as_str())
                .map_err(|error| CommandError::new("invalid_template", error.to_string()))?;
        }
        if let Some(enabled) = request.get("auto_summarize").and_then(Value::as_bool) {
            settings.set_auto_summarize(enabled);
        }
        Ok(())
    })?;
    Ok(settings_json(services, &settings))
}

/// Sends a fixed one-line request, holding no meeting content, to check a connection.
fn test_connection(services: &LlmServices, id: &str) -> Result<Value, CommandError> {
    let settings = services.settings();
    let connection = settings
        .connection(id)
        .ok_or_else(|| CommandError::new("connection_not_found", "there is no such connection"))?;
    let completion = services.engine(connection)?.complete(&LlmRequest {
        system: String::new(),
        turns: vec![LlmTurn {
            role: LlmRole::User,
            text: String::from("Reply with the single word OK."),
        }],
        images: Vec::new(),
        max_output_tokens: 64,
    })?;
    Ok(json!({"reply": completion.text.chars().take(200).collect::<String>()}))
}

/// Describes a stored summary.
pub fn summary_json(summary: &MeetingSummary) -> Value {
    json!({
        "text": summary.text,
        "connection_label": summary.connection_label,
        "model": summary.model,
        "created_at": summary.created_at.to_rfc3339(),
        "prompt": summary.prompt,
        "edited": summary.edited,
        "truncated": summary.truncated,
    })
}

/// Writes a meeting's hand-off to the owner's own AI app (#114).
///
/// Inputs: The services and a `write` step given the glossary, this device's name and the
/// owner's default template; the caller holds its repository only inside it. Outputs:
/// `{"ok":true,"message","prompt","files":[{"path","mime_type"}],"skipped"}` or the error.
/// Side effects: Those of `write`; no AI connection is used.
pub fn handoff_with(
    services: &LlmServices,
    write: impl FnOnce(
        &[String],
        &str,
        &crate::domain::llm::PromptTemplate,
    ) -> Result<crate::meeting_summaries::WrittenHandoff, SummaryError>,
) -> Value {
    let template = services.settings().template();
    match write(&services.glossary(), &services.device_name, &template) {
        Ok(written) => json!({
            "ok": true,
            "message": written.message,
            "prompt": written.prompt,
            "files": written.files.iter().map(|file| json!({
                "path": file.path.to_string_lossy(),
                "mime_type": file.mime_type,
            })).collect::<Vec<_>>(),
            "skipped": written.skipped,
        }),
        Err(error) => CommandError::from(error).to_json(),
    }
}

/// Writes a meeting's summary with a connection, the default when none is named.
///
/// Inputs: The services, the connection ID, a `prepare` step given the glossary and this
/// device's name, and a `store` step given the summary. The caller holds its repository only
/// inside those two steps. Outputs: `{"ok":true,"summary","parts","images_sent",
/// "images_skipped"}` or an error answer. Side effects: Sends the meeting to the connection's
/// server and stores the summary. Blocks for as long as the model takes.
pub fn summarize_with(
    services: &LlmServices,
    connection_id: Option<&str>,
    prepare: impl FnOnce(&[String], &str) -> Result<PreparedSummary, SummaryError>,
    store: impl FnOnce(&MeetingSummary) -> Result<(), SummaryError>,
) -> Value {
    let run = || -> Result<Value, CommandError> {
        let settings = services.settings();
        let connection = match connection_id {
            Some(id) => settings.connection(id).ok_or_else(|| {
                CommandError::new("connection_not_found", "there is no such connection")
            })?,
            None => settings.default_connection().ok_or_else(|| {
                CommandError::new(
                    "no_connection",
                    "add an AI connection in Settings, or choose a default one",
                )
            })?,
        };
        let engine = services.engine(connection)?;
        let prepared = prepare(&services.glossary(), &services.device_name)?;
        let outcome = run_summary(
            engine.as_ref(),
            connection,
            &settings.template(),
            prepared,
            chrono::Utc::now(),
        )?;
        store(&outcome.summary)?;
        Ok(json!({
            "ok": true,
            "summary": summary_json(&outcome.summary),
            "parts": outcome.parts,
            "images_sent": outcome.images_sent,
            "images_skipped": outcome.images_skipped,
        }))
    };
    run().unwrap_or_else(|error| error.to_json())
}
