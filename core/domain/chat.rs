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
        }
    }
}
