//! Reads the text of attached documents (#128): text, Markdown, CSV and JSON files, e-mails,
//! PDFs, and Word, PowerPoint, Excel and LibreOffice documents. Summaries, chat and search use
//! it; pictures go to the model as images instead.

/// Turns a file's bytes into plain text by its type.
pub mod text_extraction;
#[cfg(test)]
mod text_extraction_tests;
