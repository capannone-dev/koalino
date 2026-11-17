//! Specifies how a meeting summary (Markdown) is exported as PDF, plain text, Word or Markdown
//! (#130).

use std::{
    io::{Cursor, Read},
    path::Path,
};

use super::{
    pdf_fonts::PdfFontConfig,
    summary_export::{
        SummaryBlock, SummaryExportFormat, SummaryRun, parse_summary_blocks, render_summary,
        summary_file_name,
    },
    tests::page_texts,
};

/// A summary like the ones the models write: headings, bold labels, nested and numbered lists.
const SUMMARY: &str = "## Decisions\n\n\
- **Budget:** approved for Q4 & Q1\n\
- Hiring\n  - two engineers\n\n\
## Next steps\n\n\
1. Ada sends the plan\n\
2. Alan books the room\n\n\
Café meeting closed at 12:00. 项目测试\n";

/// The bundled fonts plus the CJK fixture fonts, nothing from the host.
fn fixture_font_config() -> PdfFontConfig {
    PdfFontConfig::bundled().with_fallback_directories(vec![
        Path::new(env!("CARGO_MANIFEST_DIR")).join("core/adapters/export/test_fonts"),
    ])
}

/// Renders the sample summary in `format`.
fn rendered(format: SummaryExportFormat) -> Vec<u8> {
    render_summary(format, "Weekly sync", SUMMARY, &fixture_font_config())
        .unwrap_or_else(|error| panic!("{format:?} export must render: {error}"))
}

/// Plain runs.
fn plain(text: &str) -> Vec<SummaryRun> {
    vec![SummaryRun::plain(text)]
}

/// Reads one part of a Word package.
fn docx_part(docx_bytes: &[u8], part_name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(docx_bytes))
        .unwrap_or_else(|error| panic!("a .docx must be a zip archive: {error}"));
    let mut part = archive
        .by_name(part_name)
        .unwrap_or_else(|error| panic!("{part_name} must exist: {error}"));
    let mut text = String::new();
    part.read_to_string(&mut text)
        .unwrap_or_else(|error| panic!("{part_name} must be UTF-8: {error}"));
    text
}

#[test]
fn markdown_becomes_headings_paragraphs_and_list_items() {
    let blocks = parse_summary_blocks(SUMMARY);
    assert_eq!(
        blocks,
        vec![
            SummaryBlock::Heading {
                level: 2,
                runs: plain("Decisions")
            },
            SummaryBlock::ListItem {
                depth: 0,
                marker: String::from("•"),
                runs: vec![
                    SummaryRun::bold("Budget:"),
                    SummaryRun::plain(" approved for Q4 & Q1"),
                ],
            },
            SummaryBlock::ListItem {
                depth: 0,
                marker: String::from("•"),
                runs: plain("Hiring"),
            },
            SummaryBlock::ListItem {
                depth: 1,
                marker: String::from("•"),
                runs: plain("two engineers"),
            },
            SummaryBlock::Heading {
                level: 2,
                runs: plain("Next steps")
            },
            SummaryBlock::ListItem {
                depth: 0,
                marker: String::from("1."),
                runs: plain("Ada sends the plan"),
            },
            SummaryBlock::ListItem {
                depth: 0,
                marker: String::from("2."),
                runs: plain("Alan books the room"),
            },
            SummaryBlock::Paragraph {
                runs: plain("Café meeting closed at 12:00. 项目测试")
            },
        ]
    );
}

#[test]
fn loose_lists_tables_and_line_breaks_keep_their_words() {
    let blocks = parse_summary_blocks(
        "- first\n\n- second\n  continued\n\n| Who | What |\n| --- | --- |\n| Ada | plan |\n",
    );
    assert_eq!(
        blocks,
        vec![
            SummaryBlock::ListItem {
                depth: 0,
                marker: String::from("•"),
                runs: plain("first"),
            },
            SummaryBlock::ListItem {
                depth: 0,
                marker: String::from("•"),
                runs: plain("second continued"),
            },
            SummaryBlock::Paragraph {
                runs: vec![SummaryRun::bold("Who | What")]
            },
            SummaryBlock::Paragraph {
                runs: plain("Ada | plan")
            },
        ]
    );
}

#[test]
fn markdown_export_is_the_summary_under_the_meeting_title() {
    let markdown = String::from_utf8(rendered(SummaryExportFormat::Markdown))
        .unwrap_or_else(|error| panic!("Markdown must be UTF-8: {error}"));
    assert_eq!(markdown, format!("# Weekly sync\n\n{SUMMARY}"));
}

#[test]
fn text_export_drops_the_markup_and_keeps_the_structure() {
    let text = String::from_utf8(rendered(SummaryExportFormat::Text))
        .unwrap_or_else(|error| panic!("text must be UTF-8: {error}"));
    assert_eq!(
        text,
        "Weekly sync\n===========\n\n\
         Decisions\n---------\n\n\
         - Budget: approved for Q4 & Q1\n\
         - Hiring\n  - two engineers\n\n\
         Next steps\n----------\n\n\
         1. Ada sends the plan\n\
         2. Alan books the room\n\n\
         Café meeting closed at 12:00. 项目测试\n"
    );
}

#[test]
fn word_export_is_a_document_with_heading_styles_and_bold_runs() {
    let docx_bytes = rendered(SummaryExportFormat::Docx);
    let content_types = docx_part(&docx_bytes, "[Content_Types].xml");
    assert!(content_types.contains("wordprocessingml.document.main+xml"));
    assert!(docx_part(&docx_bytes, "_rels/.rels").contains("word/document.xml"));
    let styles = docx_part(&docx_bytes, "word/styles.xml");
    assert!(styles.contains(r#"w:styleId="Heading2""#));
    let document = docx_part(&docx_bytes, "word/document.xml");
    assert!(document.contains(r#"<w:pStyle w:val="Title"/>"#));
    assert!(document.contains("Weekly sync"));
    assert!(document.contains(r#"<w:pStyle w:val="Heading2"/>"#));
    assert!(document.contains("<w:b/>"), "the bold label stays bold");
    assert!(
        document.contains("approved for Q4 &amp; Q1"),
        "text is escaped"
    );
    assert!(document.contains("项目测试"));
    assert!(document.contains("2."), "numbered items keep their numbers");
}

#[test]
fn pdf_export_draws_title_headings_list_items_and_any_script() {
    let pdf_bytes = rendered(SummaryExportFormat::Pdf);
    let pdf_document = lopdf::Document::load_mem(&pdf_bytes)
        .unwrap_or_else(|error| panic!("the PDF must parse: {error}"));
    let text = page_texts(&pdf_document).concat();
    for expected in [
        "Weekly sync",
        "Decisions",
        "• Budget: approved for Q4 & Q1",
        "• two engineers",
        "1. Ada sends the plan",
        "Café meeting closed at 12:00. 项目测试",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in {text:?}");
    }
}

#[test]
fn a_long_summary_continues_on_more_pages() {
    let long_summary = "- a point to discuss at length\n".repeat(200);
    let pdf_bytes = render_summary(
        SummaryExportFormat::Pdf,
        "Long",
        &long_summary,
        &PdfFontConfig::bundled(),
    )
    .unwrap_or_else(|error| panic!("PDF must render: {error}"));
    let pdf_document = lopdf::Document::load_mem(&pdf_bytes)
        .unwrap_or_else(|error| panic!("the PDF must parse: {error}"));
    assert!(pdf_document.get_pages().len() > 2);
}

#[test]
fn format_names_cover_what_people_ask_for() {
    for (name, format) in [
        ("pdf", SummaryExportFormat::Pdf),
        ("txt", SummaryExportFormat::Text),
        ("text", SummaryExportFormat::Text),
        ("docx", SummaryExportFormat::Docx),
        ("doc", SummaryExportFormat::Docx),
        ("word", SummaryExportFormat::Docx),
        ("md", SummaryExportFormat::Markdown),
        ("Markdown", SummaryExportFormat::Markdown),
    ] {
        assert_eq!(
            name.parse::<SummaryExportFormat>().ok(),
            Some(format),
            "{name}"
        );
    }
    assert!("rtf".parse::<SummaryExportFormat>().is_err());
    assert_eq!(SummaryExportFormat::Docx.file_extension(), "docx");
    assert_eq!(SummaryExportFormat::Text.file_extension(), "txt");
    assert_eq!(
        SummaryExportFormat::Docx.content_type(),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
}

#[test]
fn the_file_is_named_after_the_meeting() {
    assert_eq!(
        summary_file_name("Sync: plans/Q4", SummaryExportFormat::Pdf),
        "Sync- plans-Q4 - summary.pdf"
    );
    assert_eq!(
        summary_file_name("  ", SummaryExportFormat::Markdown),
        "Meeting - summary.md"
    );
}
