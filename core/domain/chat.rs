//! Chat with notes (#97): what a chat is about and what it said.
//!
//! A chat is about one meeting, one project, or everything (meetings, summaries and projects).
//! Each device keeps its own chats, sealed; they are not synced. Messages carry no secrets, but
//! they quote meeting content, so they are never logged.

use chrono::{DateTime, Utc};

use crate::domain::{models::MeetingId, projects::ProjectId};

/// The longest question accepted, in characters.
pub const MAX_CHAT_QUESTION_CHARS: usize = 4_000;

/// Messages kept per chat; older ones are dropped when more are added.
pub const MAX_CHAT_MESSAGES: usize = 200;

/// What a chat is about.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatScope {
    /// One meeting: its transcript, summary, notes and photos.
    Meeting(MeetingId),
    /// One project: its meetings, notes and files.
    Project(ProjectId),
    /// Every meeting, summary and project.
    All,
}

impl ChatScope {
    /// The key a chat is stored under: `meeting:<id>`, `project:<id>` or `all`.
    pub fn key(&self) -> String {
        match self {
            Self::Meeting(meeting_id) => format!("meeting:{}", meeting_id.as_str()),
            Self::Project(project_id) => format!("project:{project_id}"),
            Self::All => String::from("all"),
        }
    }

    /// Reads a scope from its kind (`meeting`, `project`, `all`) and ID.
    ///
    /// Outputs: The scope, or `None` for an unknown kind, a missing ID or a malformed project ID.
    /// Side effects: None.
    pub fn parse(kind: &str, id: Option<&str>) -> Option<Self> {
        let id = id.map(str::trim).filter(|id| !id.is_empty());
        match (kind, id) {
            ("meeting", Some(id)) => Some(Self::Meeting(MeetingId::new(id))),
            ("project", Some(id)) => uuid::Uuid::parse_str(id)
                .ok()
                .map(|uuid| Self::Project(ProjectId::from_uuid(uuid))),
            ("all", _) => Some(Self::All),
            _ => None,
        }
    }
}

/// Who wrote a message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatRole {
    /// The owner.
    User,
    /// The model.
    Assistant,
}

impl ChatRole {
    /// `user` or `assistant`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    /// Reads `user` or `assistant`.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            _ => None,
        }
    }
}

/// One message of a chat.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
    pub created_at: DateTime<Utc>,
    /// For an answer: what the model was given, such as `Budget review (full transcript)`.
    pub included: Vec<String>,
    /// For an answer: what did not fit and was left out.
    pub omitted: Vec<String>,
    /// For an answer: the connection's label.
    pub connection_label: Option<String>,
    /// For an answer: the places it cites (#110).
    pub sources: Vec<ChatSource>,
}

impl ChatMessage {
    /// The owner's question.
    pub fn question(text: &str, created_at: DateTime<Utc>) -> Self {
        Self {
            role: ChatRole::User,
            text: text.to_owned(),
            created_at,
            included: Vec::new(),
            omitted: Vec::new(),
            connection_label: None,
            sources: Vec::new(),
        }
    }
}

/// What a piece of chat material is, so a citation can open it (#110).
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatTarget {
    /// A meeting, by ID.
    Meeting {
        /// The meeting's ID.
        meeting_id: String,
    },
    /// A project file, by its resource ID.
    File {
        /// The project holding it, when it is in one.
        project_id: Option<String>,
        /// The file's resource ID.
        resource_id: String,
    },
}

/// One numbered piece of material as the model saw it: `[1]` is the first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CitedMaterial {
    /// Its label, such as `Meeting "Budget review", 2026-11-09 10:00 UTC`.
    pub label: String,
    /// What it opens; `None` for material with nothing to open, such as the list of meetings.
    pub target: Option<ChatTarget>,
}

/// A place an answer cites (#110).
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatSource {
    /// The material's number in the answer, from 1.
    pub number: u16,
    /// The material's label.
    pub label: String,
    /// What it opens.
    pub target: Option<ChatTarget>,
    /// The moment in a transcript, when the citation gives one.
    pub at_ms: Option<u64>,
}

/// The longest text read as one citation, between `[` and `]`.
const MAX_CITATION_CHARS: usize = 40;

/// Reads `h:mm:ss`, `hh:mm:ss` or `mm:ss` as milliseconds.
fn parse_time(text: &str) -> Option<u64> {
    let parts: Vec<&str> = text.split(':').collect();
    if !(2..=3).contains(&parts.len())
        || parts.iter().any(|part| {
            part.is_empty() || part.len() > 2 || !part.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return None;
    }
    let numbers: Vec<u64> = parts.iter().filter_map(|part| part.parse().ok()).collect();
    let (hours, minutes, seconds) = match numbers.as_slice() {
        [minutes, seconds] => (0, *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return None,
    };
    (minutes < 60 && seconds < 60).then_some(((hours * 60 + minutes) * 60 + seconds) * 1_000)
}

/// Reads the inside of one bracket: numbers, each optionally followed by a time.
///
/// Outputs: `(number, time)` pairs, or `None` when anything else is inside.
fn parse_bracket(inside: &str) -> Option<Vec<(u16, Option<u64>)>> {
    let mut cited: Vec<(u16, Option<u64>)> = Vec::new();
    for part in inside.split(',').map(str::trim) {
        if let Ok(number) = part.parse::<u16>() {
            cited.push((number, None));
        } else if let (Some(at_ms), Some(last)) = (parse_time(part), cited.last_mut()) {
            last.1 = Some(at_ms);
        } else {
            return None;
        }
    }
    (!cited.is_empty()).then_some(cited)
}

/// Finds the citations in an answer: `[1]`, `[1, 2]`, `[1, 00:12:34]` or `[2, 12:34]`.
///
/// Inputs: The answer and the numbered material, `[1]` first. Outputs: The places cited, in
/// order of first mention, each `(number, time)` once. Numbers outside the material, Markdown
/// links (`[text](url)`) and other bracketed text are ignored. Side effects: None.
pub fn parse_citations(answer: &str, material: &[CitedMaterial]) -> Vec<ChatSource> {
    let mut sources: Vec<ChatSource> = Vec::new();
    let mut rest = answer;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after
            .find(['[', ']'])
            .filter(|&at| after[at..].starts_with(']'))
        else {
            rest = after;
            continue;
        };
        let inside = &after[..close];
        let following = &after[close + 1..];
        rest = following;
        if inside.chars().count() > MAX_CITATION_CHARS || following.starts_with('(') {
            continue;
        }
        for (number, at_ms) in parse_bracket(inside).unwrap_or_default() {
            let Some(item) = usize::from(number)
                .checked_sub(1)
                .and_then(|index| material.get(index))
            else {
                continue;
            };
            if sources
                .iter()
                .any(|source| source.number == number && source.at_ms == at_ms)
            {
                continue;
            }
            sources.push(ChatSource {
                number,
                label: item.label.clone(),
                target: item.target.clone(),
                at_ms,
            });
        }
    }
    sources
}
