//! Describes the LLM connections that summarize meetings and answer chat (#95): which endpoint a
//! connection may use, how a connection is checked, and the summary prompt template.
//!
//! Connections need no API key (the owner removed keys from the app):
//! - OpenAI-compatible Chat Completions on a server the owner runs, such as Ollama (`/v1`),
//!   llama.cpp, LM Studio and vLLM;
//! - Claude Code and Codex, signed in on this computer with the owner's own plan (#105).
//!
//! Meeting text leaves the device only to a connection the owner set up. HTTPS is required,
//! except plain HTTP to a machine on the local network (loopback, RFC 1918 and IPv6 unique-local
//! addresses, and `.local`, `.lan` and `.home.arpa` names), where a compute node often has no
//! certificate. Everything here is pure.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Fewest context tokens a connection may declare; smaller models cannot hold a meeting.
pub const MIN_CONTEXT_TOKENS: u32 = 2_048;
/// Most context tokens a connection may declare.
pub const MAX_CONTEXT_TOKENS: u32 = 2_000_000;
/// Longest label, model name or template, in characters.
const MAX_FIELD_CHARACTERS: usize = 200;
/// Longest prompt template, in characters.
pub const MAX_TEMPLATE_CHARACTERS: usize = 8_000;

/// The placeholders a summary template may use.
pub const PLACEHOLDERS: [&str; 9] = [
    "title", "date", "time", "duration", "device", "project", "speakers", "glossary", "language",
];

/// The template used until the owner writes one.
pub const DEFAULT_SUMMARY_TEMPLATE: &str = "You summarize the meeting \"{{title}}\" held on {{date}} at {{time}} \
({{duration}}, recorded on {{device}}, project: {{project}}). Speakers: {{speakers}}. \
Names and terms to spell as written: {{glossary}}.\n\
Write the summary in the meeting's language ({{language}}) as Markdown with these sections: \
a short overview, the decisions taken, the action items with their owners and due dates when \
said, and the open questions. Mark anything uncertain in the transcript as uncertain instead of \
guessing. Use the attached pictures (whiteboards, slides) when they add information.";

/// The protocol a connection speaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmKind {
    /// Chat Completions at `<base_url>/chat/completions`.
    OpenAiCompatible,
    /// The Claude Code CLI installed and signed in on this computer (#105), run headlessly so a
    /// Claude subscription answers instead of API credit. No address and no key.
    ClaudeCode,
    /// The Codex CLI installed and signed in on this computer (#105), run headlessly so a
    /// ChatGPT subscription answers. No address and no key.
    Codex,
}

impl LlmKind {
    /// Whether this kind runs a command-line tool on this computer instead of calling a server.
    ///
    /// Outputs: `true` for Claude Code and Codex. Side effects: None.
    pub fn is_local_cli(self) -> bool {
        matches!(self, Self::ClaudeCode | Self::Codex)
    }
}

/// Why an endpoint was refused.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EndpointError {
    /// Not `http://` or `https://` followed by a host.
    #[error("the address must start with https:// and name a server")]
    Malformed,
    /// User names or passwords in the address would end up in logs and settings.
    #[error("the address must not hold a user name or password")]
    Credentials,
    /// Plain HTTP to a machine outside the local network.
    #[error("plain http:// is allowed only for a machine on your local network; use https://")]
    PlainHttpOutsideLan,
}

/// A checked endpoint, without a trailing slash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    url: String,
    plain_http: bool,
}

impl Endpoint {
    /// The URL. Side effects: None.
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// Whether it uses plain HTTP (allowed only on the local network; the apps warn about it).
    pub fn is_plain_http(&self) -> bool {
        self.plain_http
    }
}

impl std::ops::Deref for Endpoint {
    type Target = str;
    fn deref(&self) -> &str {
        &self.url
    }
}

/// Checks an endpoint against the rule above.
///
/// Inputs: The address the owner typed. Outputs: The trimmed endpoint without a trailing slash,
/// or why it was refused. Side effects: None (no name is resolved).
pub fn check_endpoint(raw: &str) -> Result<Endpoint, EndpointError> {
    let url = raw.trim().trim_end_matches('/');
    let (plain_http, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (false, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (true, rest)
    } else {
        return Err(EndpointError::Malformed);
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return Err(EndpointError::Credentials);
    }
    let host = host_of(authority).ok_or(EndpointError::Malformed)?;
    if plain_http && !is_local_host(&host) {
        return Err(EndpointError::PlainHttpOutsideLan);
    }
    if url.chars().any(char::is_whitespace) {
        return Err(EndpointError::Malformed);
    }
    Ok(Endpoint {
        url: url.to_owned(),
        plain_http,
    })
}

/// Takes the host from `host[:port]` or `[ipv6][:port]`, lowercased.
fn host_of(authority: &str) -> Option<String> {
    let (host, port) = if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, after) = bracketed.split_once(']')?;
        (host, after.strip_prefix(':'))
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    if let Some(port) = port
        && (port.is_empty() || port.parse::<u16>().is_err())
    {
        return None;
    }
    let valid = !host.is_empty()
        && host
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-:".contains(character));
    valid.then(|| host.to_ascii_lowercase())
}

/// Whether a host is on the local network.
fn is_local_host(host: &str) -> bool {
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => is_local_v4(address),
        Ok(IpAddr::V6(address)) => is_local_v6(address),
        Err(_) => {
            host == "localhost"
                || [".localhost", ".local", ".lan", ".home.arpa"]
                    .iter()
                    .any(|suffix| host.ends_with(suffix))
        }
    }
}

/// Loopback and RFC 1918 private IPv4 addresses.
fn is_local_v4(address: Ipv4Addr) -> bool {
    address.is_loopback() || address.is_private()
}

/// Loopback and unique-local (`fc00::/7`) IPv6 addresses.
fn is_local_v6(address: Ipv6Addr) -> bool {
    address.is_loopback() || (address.segments()[0] & 0xfe00) == 0xfc00
}

/// Why a connection was refused.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LlmConnectionError {
    /// The ID is not 1 to 64 letters, digits or dashes; it names files and settings entries.
    #[error("the connection ID may hold only letters, digits and dashes")]
    InvalidId,
    /// The label is empty or too long.
    #[error("give the connection a name of at most 200 characters")]
    MissingLabel,
    /// The model is empty or too long, or, for Claude Code and Codex, not a plain model name.
    #[error("name the model, for example gpt-4o-mini or llama3.1:8b")]
    MissingModel,
    /// The context size is out of range.
    #[error("the context size must be between 2048 and 2000000 tokens")]
    ContextSize,
    /// The endpoint was refused.
    #[error(transparent)]
    Endpoint(#[from] EndpointError),
}

/// One LLM connection.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LlmConnection {
    id: String,
    label: String,
    kind: LlmKind,
    base_url: String,
    model: String,
    context_tokens: u32,
}

impl LlmConnection {
    /// Checks and creates a connection with a new ID.
    ///
    /// Inputs: The label, protocol, endpoint, model and context size in tokens. For Claude Code
    /// and Codex the endpoint is ignored and the model may be empty (the tool's default).
    /// Outputs: The connection, or the first problem found. Side effects: Draws a random ID.
    pub fn new(
        label: &str,
        kind: LlmKind,
        base_url: &str,
        model: &str,
        context_tokens: u32,
    ) -> Result<Self, LlmConnectionError> {
        Self::with_id(
            &uuid::Uuid::new_v4().to_string(),
            label,
            kind,
            base_url,
            model,
            context_tokens,
        )
    }

    /// Checks and creates a connection keeping `id`, as an edit does.
    ///
    /// Outputs: The connection, or the first problem found. Side effects: None.
    pub fn with_id(
        id: &str,
        label: &str,
        kind: LlmKind,
        base_url: &str,
        model: &str,
        context_tokens: u32,
    ) -> Result<Self, LlmConnectionError> {
        if id.is_empty()
            || id.len() > 64
            || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(LlmConnectionError::InvalidId);
        }
        let label = label.trim();
        if label.is_empty() || label.chars().count() > MAX_FIELD_CHARACTERS {
            return Err(LlmConnectionError::MissingLabel);
        }
        let model = model.trim();
        let model_ok = if kind.is_local_cli() {
            // Empty leaves the choice to the tool. A name goes on the tool's command line, so it
            // may not start with `-` or hold anything but a plain model name.
            model.is_empty()
                || (model.chars().count() <= MAX_FIELD_CHARACTERS
                    && !model.starts_with('-')
                    && model.chars().all(|character| {
                        character.is_ascii_alphanumeric() || "-._:/[]".contains(character)
                    }))
        } else {
            !model.is_empty()
                && model.chars().count() <= MAX_FIELD_CHARACTERS
                && !model.chars().any(char::is_control)
        };
        if !model_ok {
            return Err(LlmConnectionError::MissingModel);
        }
        if !(MIN_CONTEXT_TOKENS..=MAX_CONTEXT_TOKENS).contains(&context_tokens) {
            return Err(LlmConnectionError::ContextSize);
        }
        // A local tool has no address; whatever was sent is dropped.
        let base_url = if kind.is_local_cli() {
            String::new()
        } else {
            check_endpoint(base_url)?.as_str().to_owned()
        };
        Ok(Self {
            id: id.to_owned(),
            label: label.to_owned(),
            kind,
            base_url,
            model: model.to_owned(),
            context_tokens,
        })
    }

    /// Checks a connection read back from settings, as `with_id` does.
    ///
    /// Outputs: The same connection, or why it is no longer valid. Side effects: None.
    pub fn revalidated(self) -> Result<Self, LlmConnectionError> {
        Self::with_id(
            &self.id,
            &self.label,
            self.kind,
            &self.base_url,
            &self.model,
            self.context_tokens,
        )
    }

    /// The stable ID. Side effects: None.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The owner's name for it.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The protocol.
    pub fn kind(&self) -> LlmKind {
        self.kind
    }

    /// The endpoint, without a trailing slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The model name sent with each request.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// How many tokens the model can read and write in one request.
    pub fn context_tokens(&self) -> u32 {
        self.context_tokens
    }

    /// Whether it uses plain HTTP on the local network.
    pub fn is_plain_http(&self) -> bool {
        self.base_url.starts_with("http://")
    }
}

/// Why a template was refused.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PromptTemplateError {
    /// Empty after trimming.
    #[error("the prompt is empty")]
    Empty,
    /// Longer than `MAX_TEMPLATE_CHARACTERS`.
    #[error("the prompt can have at most 8000 characters")]
    TooLong,
    /// `{{` without its `}}`.
    #[error("a placeholder is not closed with }}}}")]
    Unclosed,
    /// A placeholder outside `PLACEHOLDERS`.
    #[error("unknown placeholder {{{{{0}}}}}")]
    UnknownPlaceholder(String),
}

/// A checked summary prompt template.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptTemplate(String);

/// The values a template is filled with.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PromptValues {
    /// The meeting's title.
    pub title: String,
    /// Its start date, `YYYY-MM-DD`.
    pub date: String,
    /// Its start time, `HH:MM`.
    pub time: String,
    /// Its length in words, rounded to the nearest minute (at least 1), such as `45 min`.
    pub duration: String,
    /// The devices that recorded it, such as `Phone` or `Laptop + Phone` (#119).
    pub device: String,
    /// Its project, or `None`.
    pub project: Option<String>,
    /// The speakers' names in order of first appearance.
    pub speakers: Vec<String>,
    /// The glossary terms.
    pub glossary: Vec<String>,
    /// The language codes it was transcribed in, such as `it`.
    pub language: String,
}

impl PromptTemplate {
    /// Checks a template.
    ///
    /// Inputs: The text. Outputs: The template, or why it was refused. Side effects: None.
    pub fn parse(text: &str) -> Result<Self, PromptTemplateError> {
        if text.trim().is_empty() {
            return Err(PromptTemplateError::Empty);
        }
        if text.chars().count() > MAX_TEMPLATE_CHARACTERS {
            return Err(PromptTemplateError::TooLong);
        }
        let mut rest = text;
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            let end = after.find("}}").ok_or(PromptTemplateError::Unclosed)?;
            let name = after[..end].trim();
            if !PLACEHOLDERS.contains(&name) {
                return Err(PromptTemplateError::UnknownPlaceholder(name.to_owned()));
            }
            rest = &after[end + 2..];
        }
        Ok(Self(text.to_owned()))
    }

    /// The default template.
    pub fn default_summary() -> Self {
        Self(DEFAULT_SUMMARY_TEMPLATE.to_owned())
    }

    /// The template's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Fills the placeholders.
    ///
    /// Inputs: The values; empty lists read `none`. Outputs: The prompt. Side effects: None.
    pub fn render(&self, values: &PromptValues) -> String {
        let list = |items: &[String]| {
            if items.is_empty() {
                String::from("none")
            } else {
                items.join(", ")
            }
        };
        let mut rendered = String::with_capacity(self.0.len() + 128);
        let mut rest = self.0.as_str();
        while let Some(start) = rest.find("{{") {
            rendered.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else {
                break;
            };
            let value = match after[..end].trim() {
                "title" => values.title.clone(),
                "date" => values.date.clone(),
                "time" => values.time.clone(),
                "duration" => values.duration.clone(),
                "device" => values.device.clone(),
                "project" => values
                    .project
                    .clone()
                    .unwrap_or_else(|| String::from("none")),
                "speakers" => list(&values.speakers),
                "glossary" => list(&values.glossary),
                "language" => values.language.clone(),
                _ => String::new(),
            };
            rendered.push_str(&value);
            rest = &after[end + 2..];
        }
        rendered.push_str(rest);
        rendered
    }
}

/// Image types every supported provider accepts.
pub const LLM_IMAGE_MEDIA_TYPES: [&str; 4] = ["image/jpeg", "image/png", "image/webp", "image/gif"];
/// The largest image sent, so its base64 form stays under the providers' 5 MB limit.
pub const MAX_LLM_IMAGE_BYTES: usize = 3_750_000;
/// The most images sent with one request.
pub const MAX_LLM_IMAGES: usize = 20;

/// Who wrote a turn of the conversation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmRole {
    /// The owner, or the app on the owner's behalf.
    User,
    /// The model.
    Assistant,
}

/// One turn of the conversation sent to a model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlmTurn {
    /// Who wrote it.
    pub role: LlmRole,
    /// What was written.
    pub text: String,
}

/// An image sent with the first user turn, such as a meeting photo or a shared slide.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlmImage {
    media_type: &'static str,
    bytes: Vec<u8>,
}

impl LlmImage {
    /// Accepts an image the providers can read.
    ///
    /// Inputs: Its MIME type and bytes. Outputs: The image, or `None` when the type is not one of
    /// `LLM_IMAGE_MEDIA_TYPES` or it is empty or over `MAX_LLM_IMAGE_BYTES`. Side effects: None.
    pub fn new(media_type: &str, bytes: Vec<u8>) -> Option<Self> {
        let media_type = LLM_IMAGE_MEDIA_TYPES
            .into_iter()
            .find(|known| known.eq_ignore_ascii_case(media_type))?;
        (!bytes.is_empty() && bytes.len() <= MAX_LLM_IMAGE_BYTES)
            .then_some(Self { media_type, bytes })
    }

    /// Returns its MIME type. Side effects: None.
    pub fn media_type(&self) -> &'static str {
        self.media_type
    }

    /// Returns its bytes. Side effects: None.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// One request to a model: instructions, the conversation so far, and images.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlmRequest {
    /// Instructions; may be empty.
    pub system: String,
    /// The conversation, starting with a user turn and ending with one.
    pub turns: Vec<LlmTurn>,
    /// Images attached to the first user turn.
    pub images: Vec<LlmImage>,
    /// The longest answer wanted, in tokens.
    pub max_output_tokens: u32,
}

/// A model's answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LlmCompletion {
    /// The answer's text.
    pub text: String,
    /// The answer stopped at the length limit.
    pub truncated: bool,
}

/// A meeting's summary as stored in its document, which syncs it to paired devices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingSummary {
    /// The summary, in Markdown.
    pub text: String,
    /// The connection's label when it was written.
    pub connection_label: String,
    /// The model that wrote it.
    pub model: String,
    /// When it was written or last edited.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The instructions actually sent: the template with its placeholders filled.
    pub prompt: String,
    /// The owner edited it, so automatic summaries leave it alone.
    pub edited: bool,
    /// The model stopped at its length limit, so the end may be missing.
    pub truncated: bool,
}

/// A device's request that a paired computer summarize a meeting (#107), kept in the meeting's
/// document so it syncs to the computer and waits there while the two are apart.
///
/// The computer answers with its own Claude Code, Codex or local-server connection, so a phone
/// can use the owner's plan without holding its credentials.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryRequest {
    /// The device that asked (its peer ID).
    pub requested_by: String,
    /// When it asked.
    pub requested_at: chrono::DateTime<chrono::Utc>,
    /// The asking device's own summary template; `None` lets the computer use its own.
    pub template: Option<String>,
    /// How far the request has got.
    pub state: SummaryRequestState,
}

/// How far a summary request has got.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SummaryRequestState {
    /// No computer has taken it yet.
    Waiting,
    /// A computer is summarizing the meeting.
    Working {
        /// The computer (its peer ID).
        by: String,
        /// When it started.
        since: chrono::DateTime<chrono::Utc>,
    },
    /// A computer tried and failed; the asking device may ask again.
    Failed {
        /// The computer (its peer ID).
        by: String,
        /// The summary command's error code, such as `llm_tool_failed`.
        error_code: String,
        /// When it failed.
        at: chrono::DateTime<chrono::Utc>,
    },
}
