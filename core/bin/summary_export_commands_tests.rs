//! Specifies exporting a stored meeting summary as a file (#130).

use std::sync::Arc;

use chrono::Utc;

use super::{
    meeting_summaries::store_summary,
    summary_export_commands::{SummaryExportFailure, export_summary, export_summary_to_file},
};
use crate::{
    adapters::{
        export::{pdf_fonts::PdfFontConfig, summary_export::SummaryExportFormat},
        storage::sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::{
        llm::MeetingSummary,
        models::{MeetingId, MeetingTitle},
    },
};

fn repository() -> SqliteNoteRepository {
    SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"))
}

/// A meeting titled `Budget review`, with a summary when `summary` is given.
fn meeting(repository: &SqliteNoteRepository, summary: Option<&str>) -> MeetingId {
    let meeting_id = MeetingId::new("m-1");
    let title = MeetingTitle::parse("Budget review").unwrap_or_else(|e| panic!("{e}"));
    MeetingCrdtService::new(repository)
        .create_titled_meeting(&meeting_id, &title)
        .unwrap_or_else(|e| panic!("{e}"));
    if let Some(text) = summary {
        let summary = MeetingSummary {
            text: String::from(text),
            connection_label: String::from("Claude Code"),
            model: String::from("test"),
            created_at: Utc::now(),
            prompt: String::new(),
            edited: false,
            truncated: false,
        };
        store_summary(repository, &meeting_id, &summary).unwrap_or_else(|e| panic!("{e}"));
    }
    meeting_id
}

#[test]
fn a_stored_summary_exports_under_the_meeting_title() {
    let repository = repository();
    let meeting_id = meeting(
        &repository,
        Some("## Decisions\n\n- **Budget:** approved\n"),
    );
    let exported = export_summary(
        &repository,
        &meeting_id,
        SummaryExportFormat::Text,
        &PdfFontConfig::bundled(),
    )
    .unwrap_or_else(|error| panic!("export must succeed: {error}"));
    assert_eq!(exported.file_name, "Budget review - summary.txt");
    assert_eq!(
        String::from_utf8_lossy(&exported.bytes),
        "Budget review\n=============\n\nDecisions\n---------\n\n- Budget: approved\n"
    );
}

#[test]
fn a_meeting_without_a_summary_or_an_unknown_meeting_is_said_so() {
    let repository = repository();
    let meeting_id = meeting(&repository, None);
    let fonts = PdfFontConfig::bundled();
    assert!(matches!(
        export_summary(&repository, &meeting_id, SummaryExportFormat::Pdf, &fonts),
        Err(SummaryExportFailure::NoSummary)
    ));
    assert!(matches!(
        export_summary(
            &repository,
            &MeetingId::new("missing"),
            SummaryExportFormat::Pdf,
            &fonts
        ),
        Err(SummaryExportFailure::MeetingNotFound)
    ));
    assert_eq!(SummaryExportFailure::NoSummary.code(), "no_summary");
    assert_eq!(
        SummaryExportFailure::MeetingNotFound.code(),
        "meeting_not_found"
    );
}

#[test]
fn the_app_saves_the_export_where_the_owner_chose() {
    let repository = repository();
    let meeting_id = meeting(&repository, Some("Short summary."));
    let folder = std::env::temp_dir().join(format!("summary-export-{}", uuid::Uuid::new_v4()));
    let chosen = folder.join("chosen name.docx");
    let saved = export_summary_to_file(
        &repository,
        &meeting_id,
        SummaryExportFormat::Docx,
        &chosen,
        &PdfFontConfig::bundled(),
    )
    .unwrap_or_else(|error| panic!("export must succeed: {error}"));
    assert_eq!(saved, chosen);
    let bytes = std::fs::read(&chosen).unwrap_or_else(|e| panic!("{e}"));
    assert!(bytes.starts_with(b"PK"), "a .docx is a zip archive");
    let _ = std::fs::remove_dir_all(&folder);
}
