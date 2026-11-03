//! Describes an e-mail kept as a project resource (Issue #85): what a person reads in it, and the
//! files it carries.
//!
//! An imported `.eml` file stays a resource as it is; this is the view of it that parsing gives,
//! for showing it in the app and for extracting its attachments into the same project. The
//! values come from a file the user chose, so they are bounded but otherwise untrusted: they are
//! shown as plain text, and attachment names are reduced to safe file names before anything is
//! written. Nothing here is ever logged.

use chrono::{DateTime, Utc};

/// Largest e-mail file read (50 MiB): mail servers refuse far smaller messages.
pub const MAX_EMAIL_BYTES: usize = 50 * 1024 * 1024;
/// Longest body kept, in characters; the rest is cut off with `body_truncated` set.
pub const MAX_EMAIL_BODY_CHARACTERS: usize = 1_000_000;
/// Most attachments read from one e-mail; later ones are ignored.
pub const MAX_EMAIL_ATTACHMENTS: usize = 100;

/// One person in an address header.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EmailPerson {
    /// The display name, when the header gives one.
    pub name: Option<String>,
    /// The address, when the header gives one.
    pub address: Option<String>,
}

/// One file attached to an e-mail, decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmailAttachment {
    /// The name the sender gave it, or `attachment-<n>` when it has none. Not yet a safe file
    /// name: callers reduce it before writing.
    pub file_name: String,
    /// The MIME type it declares, in lower case (`application/octet-stream` when absent).
    pub mime_type: String,
    /// The decoded contents.
    pub contents: Vec<u8>,
}

/// A parsed e-mail.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EmailMessage {
    /// The subject, when present.
    pub subject: Option<String>,
    /// The senders.
    pub from: Vec<EmailPerson>,
    /// The direct recipients.
    pub to: Vec<EmailPerson>,
    /// The copied recipients.
    pub cc: Vec<EmailPerson>,
    /// When it was sent, when the header is readable.
    pub sent_at: Option<DateTime<Utc>>,
    /// The text body; an HTML-only e-mail is turned into text.
    pub body_text: String,
    /// Whether `body_text` was cut at `MAX_EMAIL_BODY_CHARACTERS`.
    pub body_truncated: bool,
    /// The attached files, at most `MAX_EMAIL_ATTACHMENTS`.
    pub attachments: Vec<EmailAttachment>,
}
