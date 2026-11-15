//! Turns an attached file into plain text (#128), so summaries, chat and search can use what a
//! meeting's or project's documents say.
//!
//! The kind comes from the stored MIME type, or from the file name's extension for project files
//! kept as `application/octet-stream`. Text-like files are read as UTF-8, e-mails through
//! `mail-parser`, PDFs through `lopdf`, and Word, PowerPoint, Excel and LibreOffice documents
//! (zip archives of XML) through `zip` and `xml-rs`. A broken file, or a kind with no text, gives
//! `None`; nothing here panics on bad input, logs content, or touches the network. The text is
//! cut at `MAX_EXTRACTED_CHARS`, so one huge file cannot crowd out everything else.

use std::io::{Cursor, Read};

use xml::reader::{EventReader, XmlEvent};

use crate::{
    adapters::email::mail_parser_reader::MailParserEmailReader, ports::email::EmailReader,
};

/// Characters of text kept from one file, at most.
pub const MAX_EXTRACTED_CHARS: usize = 200_000;

/// Bytes of one archive member read, at most; a larger member is skipped (a zip bomb guard).
const MAX_MEMBER_BYTES: u64 = 32 * 1024 * 1024;

/// Pages of a PDF read, at most.
const MAX_PDF_PAGES: usize = 500;

/// What a file is, for reading its text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    PlainText,
    Email,
    Pdf,
    Word,
    Slides,
    Spreadsheet,
    OpenDocument,
}

/// Finds the kind from the MIME type, then from the file name's extension.
fn kind_of(mime_type: &str, file_name: &str) -> Option<Kind> {
    let by_type = match mime_type {
        "text/plain" | "text/markdown" | "text/csv" | "application/json" => Some(Kind::PlainText),
        "message/rfc822" => Some(Kind::Email),
        "application/pdf" => Some(Kind::Pdf),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            Some(Kind::Word)
        }
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            Some(Kind::Slides)
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            Some(Kind::Spreadsheet)
        }
        "application/vnd.oasis.opendocument.text"
        | "application/vnd.oasis.opendocument.presentation"
        | "application/vnd.oasis.opendocument.spreadsheet" => Some(Kind::OpenDocument),
        _ => None,
    };
    if by_type.is_some() {
        return by_type;
    }
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())?;
    match extension.as_str() {
        "txt" | "md" | "markdown" | "csv" | "tsv" | "json" | "yaml" | "yml" | "log" => {
            Some(Kind::PlainText)
        }
        "eml" => Some(Kind::Email),
        "pdf" => Some(Kind::Pdf),
        "docx" => Some(Kind::Word),
        "pptx" => Some(Kind::Slides),
        "xlsx" => Some(Kind::Spreadsheet),
        "odt" | "odp" | "ods" => Some(Kind::OpenDocument),
        _ => None,
    }
}

/// Reports whether a file of this type and name can give text, before reading it.
///
/// Side effects: None.
pub fn has_text(mime_type: &str, file_name: &str) -> bool {
    kind_of(mime_type, file_name).is_some()
}

/// Reads a file's text.
///
/// Inputs: The stored MIME type, the file's shown name (for its extension) and its clear bytes.
/// Outputs: The text, trimmed and at most `MAX_EXTRACTED_CHARS` characters; `None` for pictures
/// and other kinds without text, for a file that cannot be read, or when there are no words.
/// Side effects: None; works in memory.
pub fn extract_text(mime_type: &str, file_name: &str, contents: &[u8]) -> Option<String> {
    let text = match kind_of(mime_type, file_name)? {
        Kind::PlainText => {
            let text = String::from_utf8_lossy(contents);
            text.trim_start_matches('\u{feff}').to_owned()
        }
        Kind::Email => email_text(contents)?,
        Kind::Pdf => pdf_text(contents)?,
        Kind::Word => xml_member_text(contents, "word/document.xml", &["w:t"], &["w:p"])?,
        Kind::Slides => slides_text(contents)?,
        Kind::Spreadsheet => xml_member_text(contents, "xl/sharedStrings.xml", &["t"], &["si"])?,
        Kind::OpenDocument => xml_member_text(
            contents,
            "content.xml",
            &["text:p", "text:h"],
            &["text:p", "text:h"],
        )?,
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(MAX_EXTRACTED_CHARS).collect())
}

/// An e-mail's subject, senders and body, as search shows them.
fn email_text(contents: &[u8]) -> Option<String> {
    let email = MailParserEmailReader::new().read_email(contents).ok()?;
    let senders: Vec<String> = email
        .from
        .iter()
        .map(|person| match &person.name {
            Some(name) => format!("{name} <{}>", person.address.as_deref().unwrap_or("")),
            None => person.address.clone().unwrap_or_default(),
        })
        .collect();
    Some(format!(
        "{}\n{}\n{}",
        email.subject.unwrap_or_default(),
        senders.join(", "),
        email.body_text
    ))
}

/// A PDF's text, page by page. A malformed PDF that makes the parser panic gives `None`.
fn pdf_text(contents: &[u8]) -> Option<String> {
    std::panic::catch_unwind(|| {
        let document = lopdf::Document::load_mem(contents).ok()?;
        let pages: Vec<u32> = document
            .get_pages()
            .keys()
            .copied()
            .take(MAX_PDF_PAGES)
            .collect();
        document.extract_text(&pages).ok()
    })
    .ok()
    .flatten()
}

/// Opens an office document's archive.
fn archive(contents: &[u8]) -> Option<zip::ZipArchive<Cursor<&[u8]>>> {
    zip::ZipArchive::new(Cursor::new(contents)).ok()
}

/// Reads one archive member as text, if it is there and not too large.
fn member(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    if entry.size() > MAX_MEMBER_BYTES {
        return None;
    }
    let mut text = String::new();
    entry
        .take(MAX_MEMBER_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// The text of one XML member of an office document.
fn xml_member_text(
    contents: &[u8],
    member_name: &str,
    keep: &[&str],
    breaks: &[&str],
) -> Option<String> {
    let xml = member(&mut archive(contents)?, member_name)?;
    xml_text(&xml, keep, breaks)
}

/// Each slide's text, in slide order, headed by its number.
fn slides_text(contents: &[u8]) -> Option<String> {
    let mut archive = archive(contents)?;
    let mut slides: Vec<(u32, String)> = archive
        .file_names()
        .filter_map(|name| {
            let number = name
                .strip_prefix("ppt/slides/slide")?
                .strip_suffix(".xml")?
                .parse::<u32>()
                .ok()?;
            Some((number, name.to_owned()))
        })
        .collect();
    slides.sort();
    let mut parts = Vec::new();
    for (number, name) in slides {
        let Some(xml) = member(&mut archive, &name) else {
            continue;
        };
        if let Some(text) = xml_text(&xml, &["a:t"], &["a:p"]) {
            parts.push(format!("Slide {number}\n{text}"));
        }
    }
    Some(parts.join("\n\n"))
}

/// Collects the character data inside `keep` elements, one line per `breaks` element.
///
/// Inputs: The XML, and qualified element names (`prefix:local`, or `local` without a prefix).
/// Outputs: The non-empty lines joined by newlines, or `None` for malformed XML.
fn xml_text(xml: &str, keep: &[&str], breaks: &[&str]) -> Option<String> {
    let qualified = |name: &xml::name::OwnedName| match &name.prefix {
        Some(prefix) => format!("{prefix}:{}", name.local_name),
        None => name.local_name.clone(),
    };
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut kept_depth = 0_usize;
    for event in EventReader::new(xml.as_bytes()) {
        match event.ok()? {
            XmlEvent::StartElement { name, .. } => {
                if keep.contains(&qualified(&name).as_str()) {
                    kept_depth += 1;
                }
            }
            XmlEvent::EndElement { name } => {
                let name = qualified(&name);
                if keep.contains(&name.as_str()) {
                    kept_depth = kept_depth.saturating_sub(1);
                }
                if breaks.contains(&name.as_str()) && !line.trim().is_empty() {
                    lines.push(std::mem::take(&mut line).trim().to_owned());
                }
            }
            XmlEvent::Characters(text) | XmlEvent::CData(text) | XmlEvent::Whitespace(text)
                if kept_depth > 0 =>
            {
                line.push_str(&text);
            }
            _ => {}
        }
    }
    if !line.trim().is_empty() {
        lines.push(line.trim().to_owned());
    }
    Some(lines.join("\n"))
}
