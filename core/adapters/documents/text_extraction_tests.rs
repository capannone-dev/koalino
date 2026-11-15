//! Specifies attachment text extraction (#128): each supported kind gives its words in reading
//! order, other kinds and broken files give nothing, and long text is cut at the limit.

use std::io::{Cursor, Write as _};

use super::text_extraction::{MAX_EXTRACTED_CHARS, extract_text};

/// Builds a zip archive holding `entries` (name, contents), as office documents are.
fn zip_with(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, contents) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap_or_else(|error| panic!("{error}"));
        writer
            .write_all(contents.as_bytes())
            .unwrap_or_else(|error| panic!("{error}"));
    }
    writer
        .finish()
        .unwrap_or_else(|error| panic!("{error}"))
        .into_inner()
}

/// A one-page PDF saying `text`, made with the exporter's PDF library.
fn pdf_saying(text: &str) -> Vec<u8> {
    use printpdf::{BuiltinFont, Mm, PdfDocument};
    let (document, page, layer) = PdfDocument::new("Test", Mm(210.0), Mm(297.0), "Layer");
    let font = document
        .add_builtin_font(BuiltinFont::Helvetica)
        .unwrap_or_else(|error| panic!("{error}"));
    document
        .get_page(page)
        .get_layer(layer)
        .use_text(text, 12.0, Mm(20.0), Mm(270.0), &font);
    document
        .save_to_bytes()
        .unwrap_or_else(|error| panic!("{error}"))
}

const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const PPTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
const XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const ODT: &str = "application/vnd.oasis.opendocument.text";

#[test]
fn plain_text_and_markdown_are_read_as_they_are() {
    assert_eq!(
        extract_text("text/plain", "a.txt", "Ship on Friday".as_bytes()).as_deref(),
        Some("Ship on Friday")
    );
    assert_eq!(
        extract_text("text/markdown", "a.md", "# Plan\n- one".as_bytes()).as_deref(),
        Some("# Plan\n- one")
    );
}

#[test]
fn csv_and_json_project_files_are_read_by_their_extension() {
    let csv = extract_text(
        "application/octet-stream",
        "budget.CSV",
        b"item,cost\npens,4",
    );
    let json = extract_text("application/octet-stream", "data.json", br#"{"a":1}"#);
    let binary = extract_text("application/octet-stream", "tool.exe", b"MZ\0\0");

    assert_eq!(csv.as_deref(), Some("item,cost\npens,4"));
    assert_eq!(json.as_deref(), Some(r#"{"a":1}"#));
    assert_eq!(binary, None);
}

#[test]
fn an_email_gives_its_subject_senders_and_body() {
    let raw =
        b"From: Anna Rossi <anna@example.com>\r\nSubject: Budget\r\n\r\nWe ship on Friday.\r\n";

    let text = extract_text("message/rfc822", "mail.eml", raw).unwrap_or_default();

    assert!(text.contains("Budget"));
    assert!(text.contains("Anna Rossi"));
    assert!(text.contains("We ship on Friday."));
}

#[test]
fn a_pdf_gives_its_text() {
    let text = extract_text(
        "application/pdf",
        "plan.pdf",
        &pdf_saying("Budget 2027 approved"),
    )
    .unwrap_or_default();

    assert!(text.contains("Budget 2027 approved"), "got {text:?}");
}

#[test]
fn a_word_document_gives_its_paragraphs_in_order() {
    let docx = zip_with(&[(
        "word/document.xml",
        r#"<w:document xmlns:w="w"><w:body>
            <w:p><w:r><w:t>Agenda</w:t></w:r></w:p>
            <w:p><w:r><w:t xml:space="preserve">Ship </w:t></w:r><w:r><w:t>on Friday &amp; review</w:t></w:r></w:p>
        </w:body></w:document>"#,
    )]);

    let text = extract_text(DOCX, "notes.docx", &docx).unwrap_or_default();

    assert_eq!(text, "Agenda\nShip on Friday & review");
}

#[test]
fn slides_come_in_slide_order_with_their_numbers() {
    let slide = |words: &str| {
        format!(
            r#"<p:sld xmlns:p="p" xmlns:a="a"><a:p><a:r><a:t>{words}</a:t></a:r></a:p></p:sld>"#
        )
    };
    let pptx = zip_with(&[
        ("ppt/slides/slide10.xml", &slide("Ten")),
        ("ppt/slides/slide2.xml", &slide("Two")),
        ("ppt/slides/slide1.xml", &slide("One")),
    ]);

    let text = extract_text(PPTX, "deck.pptx", &pptx).unwrap_or_default();

    assert_eq!(text, "Slide 1\nOne\n\nSlide 2\nTwo\n\nSlide 10\nTen");
}

#[test]
fn a_spreadsheet_gives_its_text_cells() {
    let xlsx = zip_with(&[(
        "xl/sharedStrings.xml",
        r#"<sst xmlns="x"><si><t>Item</t></si><si><r><t>Pens</t></r><r><t> (blue)</t></r></si></sst>"#,
    )]);

    let text = extract_text(XLSX, "costs.xlsx", &xlsx).unwrap_or_default();

    assert_eq!(text, "Item\nPens (blue)");
}

#[test]
fn a_libreoffice_document_gives_its_paragraphs() {
    let odt = zip_with(&[(
        "content.xml",
        r#"<office:document-content xmlns:office="o" xmlns:text="t"><office:body><office:text>
            <text:h>Minutes</text:h><text:p>Agreed <text:span>today</text:span></text:p>
        </office:text></office:body></office:document-content>"#,
    )]);

    let text = extract_text(ODT, "minutes.odt", &odt).unwrap_or_default();

    assert_eq!(text, "Minutes\nAgreed today");
}

#[test]
fn pictures_unknown_and_broken_files_give_nothing() {
    assert_eq!(extract_text("image/png", "a.png", b"\x89PNG"), None);
    assert_eq!(extract_text("application/pdf", "a.pdf", b"not a pdf"), None);
    assert_eq!(extract_text(DOCX, "a.docx", b"not a zip"), None);
    assert_eq!(extract_text("text/plain", "empty.txt", b"  \n"), None);
}

#[test]
fn long_text_is_cut_at_the_limit_on_a_character_boundary() {
    let long = "è".repeat(MAX_EXTRACTED_CHARS + 10);

    let text = extract_text("text/plain", "long.txt", long.as_bytes()).unwrap_or_default();

    assert_eq!(text.chars().count(), MAX_EXTRACTED_CHARS);
}
