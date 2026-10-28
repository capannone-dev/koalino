//! Defines renderer-first contracts for the Markdown, PDF, and JSON meeting exporters.
//!
//! Fixtures are built directly as `ExportDocument` values so each exporter is tested without a
//! repository. File-writing tests use unique directories under the OS temp dir.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use chrono::{TimeZone, Utc};
use printpdf::lopdf::{self, Object};
use uuid::Uuid;

use super::{
    json_exporter::JsonMeetingExporter, markdown_exporter::MarkdownMeetingExporter,
    pdf_exporter::PdfMeetingExporter,
};
use crate::{
    domain::models::{
        ExportAttachment, ExportDocument, ExportFormat, ExportOptions, ExportTranscriptSegment,
        MeetingId, SpeakerIdentity,
    },
    ports::export::MeetingExporter,
};

/// Owns a unique temporary directory and removes it when the test finishes.
pub(super) struct TestDirectory(PathBuf);

impl TestDirectory {
    /// Creates a fresh directory whose name includes `label` for easier debugging.
    pub(super) fn new(label: &str) -> Self {
        let directory_path =
            std::env::temp_dir().join(format!("note-taker-export-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory_path)
            .unwrap_or_else(|error| panic!("fixture directory must be creatable: {error}"));
        Self(directory_path)
    }

    /// Borrows the directory path.
    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Returns the fixed fixture start instant.
pub(super) fn fixture_start() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 27, 9, 0, 0)
        .single()
        .unwrap_or_else(|| panic!("fixed fixture timestamp must be valid"))
}

/// Builds a meeting with two identified speakers, one unattributed segment past the one-hour mark,
/// and no attachments.
fn sample_export_document() -> ExportDocument {
    let alice_identity_id = Uuid::from_u128(1);
    let bob_identity_id = Uuid::from_u128(2);
    ExportDocument::new(
        MeetingId::new("00000000-0000-4000-8000-000000000026"),
        "Weekly planning",
        fixture_start(),
        vec![
            ExportTranscriptSegment::new(
                0,
                30_000,
                Some(alice_identity_id),
                "Alice",
                "We will ship the *export* feature.",
            ),
            ExportTranscriptSegment::new(
                30_000,
                90_000,
                Some(bob_identity_id),
                "Bob",
                "I will verify PDF attachments.",
            ),
            ExportTranscriptSegment::new(
                3_725_000,
                3_730_000,
                None,
                "Unattributed",
                "Closing remarks #final.",
            ),
        ],
        Vec::new(),
        vec![
            SpeakerIdentity::new(alice_identity_id, "Alice"),
            SpeakerIdentity::new(bob_identity_id, "Bob"),
        ],
    )
}

/// Builds attachment metadata as staged by the orchestrator under `staging_directory`.
fn staged_attachment(
    attachment_number: u128,
    file_name: &str,
    mime_type: &str,
    staging_directory: &Path,
) -> ExportAttachment {
    ExportAttachment::new(
        Uuid::from_u128(attachment_number),
        format!("/home/user/captures/{file_name}"),
        mime_type,
        fixture_start(),
    )
    .with_staged_path(staging_directory.join(file_name).to_string_lossy())
}

/// Writes a real, decodable 40x20 PNG for image-embedding tests.
fn write_png_fixture(image_path: &Path) {
    let fixture_image = printpdf::image_crate::RgbImage::from_pixel(
        40,
        20,
        printpdf::image_crate::Rgb([200_u8, 30_u8, 30_u8]),
    );
    fixture_image
        .save(image_path)
        .unwrap_or_else(|error| panic!("PNG fixture must be writable: {error}"));
}

/// Loads a written PDF through lopdf, proving it is structurally valid.
fn load_pdf(pdf_path: &Path) -> lopdf::Document {
    lopdf::Document::load(pdf_path)
        .unwrap_or_else(|error| panic!("generated PDF must parse with lopdf: {error}"))
}

/// One `BT … ET` text object as a viewer would copy it: its decoded text and the resource names
/// of every font it selects.
pub(super) struct ExtractedTextObject {
    pub(super) text: String,
    pub(super) font_names: BTreeSet<Vec<u8>>,
}

/// Extracts each page's text, one line per text object; index 0 is page 1.
///
/// Unlike `lopdf::Document::extract_text`, this decodes embedded (Type0/Identity-H) fonts through
/// their `ToUnicode` maps, which is exactly what makes text copyable and searchable in viewers.
pub(super) fn page_texts(pdf_document: &lopdf::Document) -> Vec<String> {
    pdf_document
        .get_pages()
        .values()
        .map(|page_id| {
            page_text_objects(pdf_document, *page_id)
                .into_iter()
                .map(|text_object| text_object.text + "\n")
                .collect()
        })
        .collect()
}

/// Decodes every text object drawn on the page `page_id`.
pub(super) fn page_text_objects(
    pdf_document: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Vec<ExtractedTextObject> {
    let unicode_maps = page_font_dictionaries(pdf_document, page_id)
        .into_iter()
        .map(|(font_name, font_dictionary)| {
            (font_name, to_unicode_map(pdf_document, font_dictionary))
        })
        .collect::<BTreeMap<_, _>>();
    let page_content = pdf_document
        .get_and_decode_page_content(page_id)
        .unwrap_or_else(|error| panic!("page content must decode: {error}"));
    let mut text_objects = Vec::new();
    let mut current_object = ExtractedTextObject {
        text: String::new(),
        font_names: BTreeSet::new(),
    };
    let mut active_font_name: Option<Vec<u8>> = None;
    for operation in page_content.operations {
        match operation.operator.as_str() {
            "Tf" => {
                active_font_name = operation
                    .operands
                    .first()
                    .and_then(|operand| operand.as_name().ok())
                    .map(<[u8]>::to_vec);
                if let Some(font_name) = &active_font_name {
                    current_object.font_names.insert(font_name.clone());
                }
            }
            "Tj" => {
                if let Some(Object::String(shown_bytes, _)) = operation.operands.first() {
                    let unicode_map = active_font_name
                        .as_ref()
                        .and_then(|font_name| unicode_maps.get(font_name))
                        .and_then(Option::as_ref);
                    current_object
                        .text
                        .push_str(&decode_shown_text(shown_bytes, unicode_map));
                }
            }
            "ET" => text_objects.push(std::mem::replace(
                &mut current_object,
                ExtractedTextObject {
                    text: String::new(),
                    font_names: BTreeSet::new(),
                },
            )),
            _ => {}
        }
    }
    text_objects
}

/// Resolves the page's font resources by name. printpdf stores the `/Font` resource dictionary as
/// an indirect reference, which `lopdf::Document::get_page_fonts` does not follow.
fn page_font_dictionaries(
    pdf_document: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> BTreeMap<Vec<u8>, &lopdf::Dictionary> {
    fn resolve_dictionary<'document>(
        pdf_document: &'document lopdf::Document,
        pdf_object: &'document Object,
    ) -> Option<&'document lopdf::Dictionary> {
        pdf_document
            .dereference(pdf_object)
            .and_then(|(_, resolved_object)| resolved_object.as_dict())
            .ok()
    }
    let font_resources = pdf_document
        .get_dictionary(page_id)
        .ok()
        .and_then(|page_dictionary| page_dictionary.get(b"Resources").ok())
        .and_then(|resources_object| resolve_dictionary(pdf_document, resources_object))
        .and_then(|resources| resources.get(b"Font").ok())
        .and_then(|fonts_object| resolve_dictionary(pdf_document, fonts_object));
    font_resources
        .map(|font_resources| {
            font_resources
                .iter()
                .filter_map(|(font_name, font_object)| {
                    resolve_dictionary(pdf_document, font_object)
                        .map(|font_dictionary| (font_name.clone(), font_dictionary))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Decodes a `Tj` operand: two-byte glyph IDs through `unicode_map` for embedded fonts, WinAnsi
/// bytes for the built-in fonts. Unmapped glyphs decode as U+FFFD.
fn decode_shown_text(shown_bytes: &[u8], unicode_map: Option<&BTreeMap<u16, String>>) -> String {
    match unicode_map {
        Some(unicode_map) => shown_bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|glyph_bytes| {
                unicode_map
                    .get(&u16::from_be_bytes(*glyph_bytes))
                    .cloned()
                    .unwrap_or_else(|| String::from('\u{FFFD}'))
            })
            .collect(),
        None => lopdf::Document::decode_text(Some("WinAnsiEncoding"), shown_bytes),
    }
}

/// Parses the `bfchar` entries of a font's `ToUnicode` CMap, or returns `None` for fonts without
/// one (the built-in fonts).
fn to_unicode_map(
    pdf_document: &lopdf::Document,
    font_dictionary: &lopdf::Dictionary,
) -> Option<BTreeMap<u16, String>> {
    let to_unicode_reference = font_dictionary.get(b"ToUnicode").ok()?;
    let (_, to_unicode_object) = pdf_document.dereference(to_unicode_reference).ok()?;
    let to_unicode_stream = to_unicode_object.as_stream().ok()?;
    let cmap_bytes = to_unicode_stream
        .decompressed_content()
        .unwrap_or_else(|_| to_unicode_stream.content.clone());
    let mut unicode_map = BTreeMap::new();
    let mut inside_bfchar_block = false;
    for cmap_line in String::from_utf8_lossy(&cmap_bytes).lines() {
        let cmap_line = cmap_line.trim();
        if cmap_line.ends_with("beginbfchar") {
            inside_bfchar_block = true;
        } else if cmap_line == "endbfchar" {
            inside_bfchar_block = false;
        } else if inside_bfchar_block {
            let hex_fields = cmap_line
                .split(['<', '>'])
                .map(str::trim)
                .filter(|field| !field.is_empty())
                .collect::<Vec<_>>();
            if let [glyph_hex, unicode_hex] = hex_fields.as_slice()
                && let Ok(glyph_id) = u16::from_str_radix(glyph_hex, 16)
            {
                unicode_map.insert(glyph_id, decode_utf16_hex(unicode_hex));
            }
        }
    }
    Some(unicode_map)
}

/// Decodes a CMap destination such as `00E9` or `D83CDF89` (UTF-16BE); anything else, including
/// a bare five-digit code point, decodes as U+FFFD because viewers cannot read it either.
fn decode_utf16_hex(unicode_hex: &str) -> String {
    if unicode_hex.is_empty() || !unicode_hex.len().is_multiple_of(4) {
        return String::from('\u{FFFD}');
    }
    let code_units = (0..unicode_hex.len())
        .step_by(4)
        .map(|offset| u16::from_str_radix(&unicode_hex[offset..offset + 4], 16))
        .collect::<Result<Vec<_>, _>>();
    code_units
        .ok()
        .and_then(|code_units| String::from_utf16(&code_units).ok())
        .unwrap_or_else(|| String::from('\u{FFFD}'))
}

/// Exports one document as PDF into `directory` and returns the parsed result.
fn export_pdf(document: &ExportDocument, directory: &Path) -> lopdf::Document {
    let output_path = directory.join("meeting.pdf");
    PdfMeetingExporter
        .export_meeting(
            document,
            &ExportOptions::for_format(ExportFormat::Pdf),
            &output_path,
        )
        .unwrap_or_else(|error| panic!("PDF export must succeed: {error}"));
    load_pdf(&output_path)
}

/// Renders Markdown for one document relative to `output_directory`.
fn render_markdown(
    document: &ExportDocument,
    options: &ExportOptions,
    output_directory: &Path,
) -> String {
    MarkdownMeetingExporter::render(std::slice::from_ref(document), options, output_directory)
}

/// Verifies the Markdown header lists title, date, duration, and participants.
#[test]
fn markdown_renders_metadata_header() {
    let rendered_markdown = render_markdown(
        &sample_export_document(),
        &ExportOptions::for_format(ExportFormat::Markdown),
        Path::new("/exports"),
    );

    assert!(rendered_markdown.starts_with("# Weekly planning\n"));
    assert!(rendered_markdown.contains("- Date: 2026-10-27T09:00:00+00:00"));
    assert!(rendered_markdown.contains("- Duration: 01:02:10"));
    assert!(rendered_markdown.contains("- Participants: Alice, Bob, Unattributed"));
}

/// Verifies each segment is attributed to its speaker with an `HH:MM:SS` start offset.
#[test]
fn markdown_attributes_speakers_and_formats_hour_timestamps() {
    let rendered_markdown = render_markdown(
        &sample_export_document(),
        &ExportOptions::for_format(ExportFormat::Markdown),
        Path::new("/exports"),
    );

    assert!(rendered_markdown.contains("**Bob** — [00:00:30] I will verify PDF attachments."));
    assert!(rendered_markdown.contains("**Unattributed** — [01:02:05] Closing remarks"));
}

/// Verifies user text cannot inject Markdown emphasis or headings.
#[test]
fn markdown_escapes_control_characters_in_transcript_text() {
    let rendered_markdown = render_markdown(
        &sample_export_document(),
        &ExportOptions::for_format(ExportFormat::Markdown),
        Path::new("/exports"),
    );

    assert!(rendered_markdown.contains("We will ship the \\*export\\* feature."));
    assert!(rendered_markdown.contains("Closing remarks \\#final."));
}

/// Verifies staged attachments are linked relative to the export file and images render inline.
#[test]
fn markdown_links_staged_attachments_with_relative_paths() {
    let output_directory = Path::new("/exports");
    let staging_directory = output_directory.join("weekly_attachments");
    let document = sample_export_document().with_attachments(vec![
        staged_attachment(26, "agenda.png", "image/png", &staging_directory),
        staged_attachment(27, "slides.pdf", "application/pdf", &staging_directory),
        ExportAttachment::new(
            Uuid::from_u128(28),
            "/home/user/missing.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            fixture_start(),
        ),
    ]);

    let rendered_markdown = render_markdown(
        &document,
        &ExportOptions::for_format(ExportFormat::Markdown),
        output_directory,
    );

    assert!(rendered_markdown.contains("## Attachments"));
    assert!(rendered_markdown.contains("![agenda.png](<weekly_attachments/agenda.png>)"));
    assert!(
        rendered_markdown
            .contains("[slides.pdf](<weekly_attachments/slides.pdf>) (application/pdf)")
    );
    assert!(rendered_markdown.contains("missing.docx"));
    assert!(rendered_markdown.contains("source file unavailable"));
    assert!(!rendered_markdown.contains("/home/user/captures"));
}

/// Verifies summary statistics report total words and per-speaker speaking time.
#[test]
fn markdown_computes_word_count_and_speaking_time() {
    let rendered_markdown = render_markdown(
        &sample_export_document(),
        &ExportOptions::for_format(ExportFormat::Markdown),
        Path::new("/exports"),
    );

    assert!(rendered_markdown.contains("## Summary statistics"));
    assert!(rendered_markdown.contains("- Word count: 14"));
    assert!(rendered_markdown.contains("- Speaking time: Alice — 00:00:30"));
    assert!(rendered_markdown.contains("- Speaking time: Bob — 00:01:00"));
}

/// Verifies disabling metadata removes the header details but keeps the transcript.
#[test]
fn markdown_omits_metadata_when_disabled() {
    let rendered_markdown = render_markdown(
        &sample_export_document(),
        &ExportOptions::new(ExportFormat::Markdown, true, false, None),
        Path::new("/exports"),
    );

    assert!(!rendered_markdown.contains("- Date:"));
    assert!(!rendered_markdown.contains("- Participants:"));
    assert!(rendered_markdown.contains("**Alice** — [00:00:00]"));
}

/// Verifies a batch export writes one file with one section per meeting and reports the count.
#[test]
fn markdown_batch_export_writes_one_section_per_meeting() {
    let test_directory = TestDirectory::new("markdown-batch");
    let retro_document = ExportDocument::new(
        MeetingId::new("retro"),
        "Retro",
        fixture_start(),
        vec![ExportTranscriptSegment::new(
            0,
            1_000,
            None,
            "Unattributed",
            "Went well.",
        )],
        Vec::new(),
        Vec::new(),
    );
    let output_path = test_directory.path().join("batch.md");

    let export_result = MarkdownMeetingExporter
        .export_meetings(
            &[sample_export_document(), retro_document],
            &ExportOptions::for_format(ExportFormat::Markdown),
            &output_path,
        )
        .unwrap_or_else(|error| panic!("batch Markdown export must succeed: {error}"));
    let written_markdown = fs::read_to_string(&output_path)
        .unwrap_or_else(|error| panic!("batch output must be readable: {error}"));

    assert!(written_markdown.starts_with("# Meeting export\n"));
    assert!(written_markdown.contains("## Weekly planning"));
    assert!(written_markdown.contains("## Retro"));
    assert!(written_markdown.contains("\n---\n"));
    assert_eq!(export_result.format(), ExportFormat::Markdown);
    assert_eq!(export_result.metadata_value("meeting_count"), Some("2"));
    assert_eq!(
        export_result.size_bytes(),
        u64::try_from(written_markdown.len()).unwrap_or_default()
    );
}

/// Verifies the PDF is parseable and has title, contents, and transcript pages with content.
#[test]
fn pdf_writes_valid_document_with_title_contents_and_transcript_pages() {
    let test_directory = TestDirectory::new("pdf-structure");

    let pdf_document = export_pdf(&sample_export_document(), test_directory.path());
    let texts = page_texts(&pdf_document);

    assert!(texts.len() >= 3);
    assert!(texts[0].contains("Weekly planning"));
    assert!(texts[0].contains("Participants: Alice, Bob, Unattributed"));
    assert!(texts[1].contains("Contents"));
    assert!(texts[2].contains("Alice"));
    assert!(texts[2].contains("[00:00:30]"));
}

/// Verifies a decodable staged image is embedded as a PDF image XObject.
#[test]
fn pdf_embeds_decodable_image_attachment_as_image_xobject() {
    let test_directory = TestDirectory::new("pdf-image");
    write_png_fixture(&test_directory.path().join("agenda.png"));
    let document = sample_export_document().with_attachments(vec![staged_attachment(
        26,
        "agenda.png",
        "image/png",
        test_directory.path(),
    )]);

    let pdf_document = export_pdf(&document, test_directory.path());

    let has_image_xobject = pdf_document.objects.values().any(|pdf_object| {
        matches!(pdf_object, Object::Stream(stream)
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image".as_slice()))
    });
    assert!(has_image_xobject);
}

/// Verifies long transcripts paginate and every page carries an accurate `Page N of M` footer.
#[test]
fn pdf_paginates_long_transcript_with_page_number_footers() {
    let test_directory = TestDirectory::new("pdf-pagination");
    let long_segments = (0..200_u64)
        .map(|segment_index| {
            ExportTranscriptSegment::new(
                segment_index * 10_000,
                segment_index * 10_000 + 9_000,
                None,
                "Unattributed",
                format!("Discussion point number {segment_index}."),
            )
        })
        .collect();
    let document = ExportDocument::new(
        MeetingId::new("long-meeting"),
        "Long meeting",
        fixture_start(),
        long_segments,
        Vec::new(),
        Vec::new(),
    );

    let pdf_document = export_pdf(&document, test_directory.path());
    let texts = page_texts(&pdf_document);
    let total_pages = texts.len();

    assert!(total_pages > 4);
    for (page_index, page_text) in texts.iter().enumerate() {
        assert!(
            page_text.contains(&format!("Page {} of {total_pages}", page_index + 1)),
            "page {} is missing its footer",
            page_index + 1
        );
    }
}

/// Verifies the contents page lists the real page number of each section.
#[test]
fn pdf_contents_page_lists_section_page_numbers() {
    let test_directory = TestDirectory::new("pdf-contents");
    write_png_fixture(&test_directory.path().join("agenda.png"));
    let document = sample_export_document().with_attachments(vec![staged_attachment(
        26,
        "agenda.png",
        "image/png",
        test_directory.path(),
    )]);

    let pdf_document = export_pdf(&document, test_directory.path());
    let texts = page_texts(&pdf_document);
    let attachments_page_number = texts
        .iter()
        .enumerate()
        .skip(2)
        .find(|(_, page_text)| page_text.contains("Attachments"))
        .map(|(page_index, _)| page_index + 1)
        .unwrap_or_else(|| panic!("an attachments section must be rendered"));

    assert!(texts[1].contains("Transcript (page 3)"));
    assert!(texts[1].contains(&format!("Attachments (page {attachments_page_number})")));
    assert!(texts[1].contains("Summary statistics (page"));
}

/// Verifies non-image attachments are embedded as PDF file attachments with their bytes.
#[test]
fn pdf_embeds_non_image_attachment_as_embedded_file() {
    let test_directory = TestDirectory::new("pdf-embedded-file");
    let attachment_bytes = b"%PDF-1.4 fixture slides";
    fs::write(test_directory.path().join("slides.pdf"), attachment_bytes)
        .unwrap_or_else(|error| panic!("attachment fixture must be writable: {error}"));
    let document = sample_export_document().with_attachments(vec![staged_attachment(
        27,
        "slides.pdf",
        "application/pdf",
        test_directory.path(),
    )]);

    let pdf_document = export_pdf(&document, test_directory.path());

    let catalog = pdf_document
        .catalog()
        .unwrap_or_else(|error| panic!("catalog must exist: {error}"));
    assert!(catalog.has(b"Names"));
    let has_named_filespec = pdf_document.objects.values().any(|pdf_object| {
        matches!(pdf_object, Object::Dictionary(dictionary)
            if dictionary.get(b"Type").and_then(Object::as_name).ok() == Some(b"Filespec".as_slice())
                && matches!(dictionary.get(b"F"), Ok(Object::String(file_name, _)) if file_name == b"slides.pdf"))
    });
    let has_embedded_bytes = pdf_document.objects.values().any(|pdf_object| {
        matches!(pdf_object, Object::Stream(stream)
            if stream.dict.get(b"Type").and_then(Object::as_name).ok() == Some(b"EmbeddedFile".as_slice())
                && stream.decompressed_content().unwrap_or_else(|_| stream.content.clone()) == attachment_bytes)
    });
    assert!(has_named_filespec);
    assert!(has_embedded_bytes);
}

/// Verifies an undecodable image is reported in the document instead of failing the export.
#[test]
fn pdf_degrades_gracefully_for_undecodable_image() {
    let test_directory = TestDirectory::new("pdf-bad-image");
    fs::write(test_directory.path().join("broken.png"), [0_u8; 64])
        .unwrap_or_else(|error| panic!("broken image fixture must be writable: {error}"));
    let document = sample_export_document().with_attachments(vec![staged_attachment(
        29,
        "broken.png",
        "image/png",
        test_directory.path(),
    )]);

    let pdf_document = export_pdf(&document, test_directory.path());

    assert!(
        page_texts(&pdf_document)
            .iter()
            .any(|page_text| page_text.contains("(image could not be embedded)"))
    );
}

/// Verifies long transcript text is wrapped so no rendered line exceeds the printable width.
#[test]
fn pdf_wraps_long_lines_within_page_width() {
    let test_directory = TestDirectory::new("pdf-wrapping");
    let long_text = "word ".repeat(120);
    let document = ExportDocument::new(
        MeetingId::new("wrapping"),
        "Wrapping",
        fixture_start(),
        vec![ExportTranscriptSegment::new(
            0,
            1_000,
            None,
            "Unattributed",
            long_text,
        )],
        Vec::new(),
        Vec::new(),
    );

    let pdf_document = export_pdf(&document, test_directory.path());
    let transcript_text = &page_texts(&pdf_document)[2];

    assert!(
        transcript_text
            .lines()
            .all(|rendered_line| rendered_line.chars().count() <= 100)
    );
    assert!(transcript_text.matches("word").count() >= 120);
}

/// Renders JSON for `documents` and parses it for assertions.
fn render_json(documents: &[ExportDocument]) -> serde_json::Value {
    let rendered_json = JsonMeetingExporter::render(
        documents,
        &ExportOptions::for_format(ExportFormat::Json),
        fixture_start(),
    )
    .unwrap_or_else(|error| panic!("JSON fixture must render: {error}"));
    serde_json::from_slice(&rendered_json)
        .unwrap_or_else(|error| panic!("JSON fixture must parse: {error}"))
}

/// Verifies the top level is versioned and always carries a `meetings` array.
#[test]
fn json_emits_schema_version_and_meetings_array() {
    let parsed_json = render_json(&[sample_export_document()]);

    assert_eq!(parsed_json["schema_version"], "1.0");
    assert_eq!(parsed_json["exported_at"], "2026-10-27T09:00:00+00:00");
    assert_eq!(parsed_json["meetings"].as_array().map(Vec::len), Some(1));
    let meeting = &parsed_json["meetings"][0]["meeting"];
    assert_eq!(
        meeting["meeting_id"],
        "00000000-0000-4000-8000-000000000026"
    );
    assert_eq!(meeting["title"], "Weekly planning");
    assert_eq!(meeting["started_at"], "2026-10-27T09:00:00+00:00");
    assert_eq!(meeting["duration_ms"], 3_730_000);
}

/// Verifies segments keep exact millisecond bounds and speaker identity references.
#[test]
fn json_preserves_segment_timing_and_speaker_identity() {
    let parsed_json = render_json(&[sample_export_document()]);
    let segments = &parsed_json["meetings"][0]["transcript_segments"];

    assert_eq!(segments[1]["start_time_ms"], 30_000);
    assert_eq!(segments[1]["end_time_ms"], 90_000);
    assert_eq!(segments[1]["speaker"]["display_name"], "Bob");
    assert_eq!(
        segments[1]["speaker"]["identity_id"],
        Uuid::from_u128(2).to_string()
    );
    assert!(segments[2]["speaker"]["identity_id"].is_null());
    assert_eq!(
        parsed_json["meetings"][0]["voice_identities"][0]["display_name"],
        "Alice"
    );
    assert_eq!(
        parsed_json["meetings"][0]["statistics"]["speaking_time_ms"]["Bob"],
        60_000
    );
    assert_eq!(parsed_json["meetings"][0]["statistics"]["word_count"], 14);
}

/// Verifies attachments are listed with their original source paths, MIME types, and timestamps.
#[test]
fn json_includes_attachment_source_paths() {
    let document = sample_export_document().with_attachments(vec![staged_attachment(
        26,
        "agenda.png",
        "image/png",
        Path::new("/tmp/staging"),
    )]);

    let parsed_json = render_json(&[document]);
    let attachment = &parsed_json["meetings"][0]["attachments"][0];

    assert_eq!(attachment["file_path"], "/home/user/captures/agenda.png");
    assert_eq!(attachment["mime_type"], "image/png");
    assert_eq!(
        attachment["original_timestamp"],
        "2026-10-27T09:00:00+00:00"
    );
    assert_eq!(attachment["attachment_id"], Uuid::from_u128(26).to_string());
}

/// Verifies biometric data never leaks into exported JSON.
#[test]
fn json_never_serializes_embeddings() {
    let rendered_json = JsonMeetingExporter::render(
        &[sample_export_document()],
        &ExportOptions::for_format(ExportFormat::Json),
        fixture_start(),
    )
    .unwrap_or_else(|error| panic!("JSON fixture must render: {error}"));
    let rendered_text = String::from_utf8_lossy(&rendered_json);

    assert!(!rendered_text.contains("embedding"));
}

/// Verifies a batch file contains every meeting and metadata can be suppressed.
#[test]
fn json_batch_contains_all_meetings_and_honors_metadata_option() {
    let test_directory = TestDirectory::new("json-batch");
    let output_path = test_directory.path().join("batch.json");

    let export_result = JsonMeetingExporter
        .export_meetings(
            &[sample_export_document(), sample_export_document()],
            &ExportOptions::new(ExportFormat::Json, true, false, None),
            &output_path,
        )
        .unwrap_or_else(|error| panic!("batch JSON export must succeed: {error}"));
    let parsed_json: serde_json::Value = serde_json::from_slice(
        &fs::read(&output_path)
            .unwrap_or_else(|error| panic!("batch output must be readable: {error}")),
    )
    .unwrap_or_else(|error| panic!("batch output must parse: {error}"));

    assert_eq!(parsed_json["meetings"].as_array().map(Vec::len), Some(2));
    assert!(parsed_json["meetings"][0]["meeting"]["title"].is_null());
    assert_eq!(
        parsed_json["meetings"][0]["meeting"]["meeting_id"],
        "00000000-0000-4000-8000-000000000026"
    );
    assert_eq!(export_result.metadata_value("schema_version"), Some("1.0"));
    assert_eq!(export_result.metadata_value("meeting_count"), Some("2"));
}

/// Verifies transparent image pixels are composited onto white rather than turning black.
#[test]
fn pdf_flattens_transparent_image_pixels_onto_white() {
    let mut transparent_image = printpdf::image_crate::RgbaImage::new(2, 1);
    transparent_image.put_pixel(0, 0, printpdf::image_crate::Rgba([0, 0, 0, 0]));
    transparent_image.put_pixel(1, 0, printpdf::image_crate::Rgba([0, 0, 255, 255]));

    let flattened_image = super::pdf_exporter::flatten_onto_white(
        printpdf::image_crate::DynamicImage::ImageRgba8(transparent_image),
    )
    .to_rgb8();

    assert_eq!(flattened_image.get_pixel(0, 0).0, [255, 255, 255]);
    assert_eq!(flattened_image.get_pixel(1, 0).0, [0, 0, 255]);
}
