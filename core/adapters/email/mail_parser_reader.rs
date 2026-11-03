//! Reads imported `.eml` e-mails with `mail-parser` (Issue #85).
//!
//! The parser is pure Rust and touches neither the network nor the disk. This adapter turns its
//! view into the domain's `EmailMessage`: decoded headers, a text body (an HTML-only e-mail is
//! turned into text, so no markup ever reaches a viewer), and decoded attachments, all within
//! the limits in `domain::email`. It never logs content, names or addresses.

use chrono::{DateTime, Utc};
use mail_parser::{Address, MessageParser, MimeHeaders};

use crate::{
    domain::email::{
        EmailAttachment, EmailMessage, EmailPerson, MAX_EMAIL_ATTACHMENTS,
        MAX_EMAIL_BODY_CHARACTERS, MAX_EMAIL_BYTES,
    },
    ports::email::{EmailReadError, EmailReader},
};

/// An `EmailReader` over `mail-parser`, with its size limits.
#[derive(Clone, Copy, Debug)]
pub struct MailParserEmailReader {
    /// Largest file read, in bytes.
    max_email_bytes: usize,
    /// Longest body kept, in characters.
    max_body_characters: usize,
}

impl MailParserEmailReader {
    /// Creates a reader with the domain limits.
    ///
    /// Outputs: The reader. Side effects: None.
    pub fn new() -> Self {
        Self::with_limits(MAX_EMAIL_BYTES, MAX_EMAIL_BODY_CHARACTERS)
    }

    /// Creates a reader with other size limits, for tests.
    ///
    /// Inputs: The largest file in bytes and the longest body in characters.
    /// Outputs: The reader. Side effects: None.
    pub fn with_limits(max_email_bytes: usize, max_body_characters: usize) -> Self {
        Self {
            max_email_bytes,
            max_body_characters,
        }
    }
}

impl Default for MailParserEmailReader {
    fn default() -> Self {
        Self::new()
    }
}

impl EmailReader for MailParserEmailReader {
    fn read_email(&self, raw_email: &[u8]) -> Result<EmailMessage, EmailReadError> {
        if raw_email.len() > self.max_email_bytes {
            return Err(EmailReadError::TooLarge);
        }
        let message = MessageParser::default()
            .parse(raw_email)
            .filter(|message| !message.headers().is_empty())
            .ok_or(EmailReadError::Malformed)?;
        let full_body = message
            .body_text(0)
            .map(|text| text.into_owned())
            .unwrap_or_default();
        let body_truncated = full_body.chars().nth(self.max_body_characters).is_some();
        let body_text = if body_truncated {
            full_body.chars().take(self.max_body_characters).collect()
        } else {
            full_body
        };
        let attachments = message
            .attachments()
            .take(MAX_EMAIL_ATTACHMENTS)
            .enumerate()
            .map(|(index, part)| EmailAttachment {
                file_name: part
                    .attachment_name()
                    .filter(|name| !name.trim().is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("attachment-{}", index + 1)),
                mime_type: part
                    .content_type()
                    .map(|content_type| match content_type.subtype() {
                        Some(subtype) => format!("{}/{subtype}", content_type.ctype()),
                        None => content_type.ctype().to_owned(),
                    })
                    .unwrap_or_else(|| String::from("application/octet-stream"))
                    .to_ascii_lowercase(),
                contents: part.contents().to_vec(),
            })
            .collect();
        Ok(EmailMessage {
            subject: message.subject().map(str::to_owned),
            from: people(message.from()),
            to: people(message.to()),
            cc: people(message.cc()),
            sent_at: message
                .date()
                .and_then(|date| DateTime::<Utc>::from_timestamp(date.to_timestamp(), 0)),
            body_text,
            body_truncated,
            attachments,
        })
    }
}

/// Lists the people of an address header, groups flattened.
///
/// Inputs: The header, if present. Outputs: Its people, in order. Side effects: None.
fn people(address: Option<&Address<'_>>) -> Vec<EmailPerson> {
    address
        .map(|address| {
            address
                .iter()
                .map(|person| EmailPerson {
                    name: person
                        .name
                        .as_deref()
                        .filter(|name| !name.trim().is_empty())
                        .map(str::to_owned),
                    address: person.address.as_deref().map(str::to_owned),
                })
                .collect()
        })
        .unwrap_or_default()
}
