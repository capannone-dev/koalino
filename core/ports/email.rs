//! Declares how imported e-mails are read (Issue #85), so the application never depends on a mail
//! parsing library.

use crate::domain::email::EmailMessage;

/// Why an e-mail could not be read.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EmailReadError {
    /// The file is larger than the reader accepts.
    #[error("the e-mail is too large to read")]
    TooLarge,
    /// The file is not an e-mail.
    #[error("the file is not a readable e-mail")]
    Malformed,
}

/// Reads raw RFC 5322 e-mails, such as `.eml` files.
pub trait EmailReader: Send + Sync {
    /// Parses one e-mail.
    ///
    /// Inputs: The file's bytes.
    /// Outputs: The headers, text body and attachments within the domain limits, or why it could
    /// not be read. Side effects: None; never logs content.
    fn read_email(&self, raw_email: &[u8]) -> Result<EmailMessage, EmailReadError>;
}
