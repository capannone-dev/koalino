//! Generates paginated A4 meeting PDFs with a title page, contents, transcript, attachments, and
//! statistics, using `printpdf` for drawing and its bundled `lopdf` for embedded files.
//!
//! Rendering is two-pass: pass one wraps text, decodes images, and assigns every item to a page so
//! the contents page and `Page N of M` footers know real page numbers; pass two draws the pages.
//! Image attachments are drawn inline; other staged attachments are embedded as PDF file
//! attachments (the viewer's attachments panel).
//!
//! Text is drawn with embedded Unicode TrueType fonts chosen per character by `pdf_fonts` (bundled
//! DejaVu Sans, then CJK/emoji fallback fonts), and wrapped by measured glyph widths. printpdf
//! subsets each font to the glyphs used. A final lopdf pass fixes what printpdf gets wrong or leaves
//! out: `ToUnicode` maps rebuilt from the subset fonts (so text copies back exactly), a UTF-16
//! document title, and file attachments.

use std::{fs, path::Path};

use std::collections::{BTreeMap, BTreeSet};

use printpdf::{
    BuiltinFont, Image, ImageTransform, IndirectFontRef, Mm, PdfDocument, PdfDocumentReference,
    TextMatrix,
    image_crate::{self, DynamicImage, GenericImageView},
    lopdf::{self, Dictionary, Object, ObjectId, Stream, StringFormat},
};

use crate::{
    domain::{
        errors::ExportError,
        models::{
            ExportAttachment, ExportDocument, ExportFormat, ExportOptions, ExportResult,
            format_export_offset,
        },
    },
    ports::export::MeetingExporter,
};

use super::{
    ensure_documents_present,
    pdf_fonts::{self, EmbeddedFonts, FontSlot, LoadedFont, PdfFontConfig, TextFonts},
    write_export_artifact,
};

/// A4 page width.
const PAGE_WIDTH_MM: f32 = 210.0;
/// A4 page height.
const PAGE_HEIGHT_MM: f32 = 297.0;
/// Left edge of all body content.
const LEFT_MARGIN_MM: f32 = 25.0;
/// Printable width between the left and right margins.
pub(crate) const TEXT_WIDTH_MM: f32 = 160.0;
/// Baseline of the first line on every page.
const CONTENT_TOP_MM: f32 = 272.0;
/// Lowest baseline allowed for body content; the footer sits below it.
const CONTENT_BOTTOM_MM: f32 = 25.0;
/// Baseline of the `Page N of M` footer.
const FOOTER_BASELINE_MM: f32 = 12.0;
/// Millimetres per typographic point.
pub(crate) const MM_PER_POINT: f32 = 0.3528;
/// Largest height an inline image may occupy.
const MAX_IMAGE_HEIGHT_MM: f32 = 150.0;
/// Resolution used to derive an image's natural size before fitting it to the page.
const IMAGE_NATURAL_DPI: f32 = 150.0;
/// Contents entries that fit on one contents page.
const CONTENTS_ENTRIES_PER_PAGE: usize = 30;

/// Body text size.
pub(crate) const BODY_FONT_SIZE: f32 = 10.0;
/// Section heading size.
const SECTION_FONT_SIZE: f32 = 14.0;
/// Meeting heading size inside a batch.
const MEETING_FONT_SIZE: f32 = 16.0;
/// Title page heading size.
const TITLE_FONT_SIZE: f32 = 24.0;

/// Produces professional, self-contained meeting PDFs from application-extracted data.
pub struct PdfMeetingExporter;

impl MeetingExporter for PdfMeetingExporter {
    fn format(&self) -> ExportFormat {
        ExportFormat::Pdf
    }

    /// Lays out, draws, and writes a PDF for one or more meetings.
    ///
    /// Inputs: Non-empty `documents` whose attachments were staged by the orchestrator, export
    /// `options`, and the destination path.
    /// Outputs: The written artifact summary or an `ExportError`.
    /// Side effects: Reads staged attachment files and, when the text needs them, fallback font
    /// files (see `PdfFontConfig::from_environment`); creates the destination directory and file.
    /// Unreadable or undecodable attachments are noted in the PDF rather than failing the export.
    fn export_meetings(
        &self,
        documents: &[ExportDocument],
        options: &ExportOptions,
        output_path: &Path,
    ) -> Result<ExportResult, ExportError> {
        ensure_documents_present(documents.len())?;
        let pdf_bytes = render_pdf(documents, options, &PdfFontConfig::from_environment())?;
        write_export_artifact(output_path, &pdf_bytes, ExportFormat::Pdf, documents.len())
    }
}

/// Selects one of the two primary faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FontWeight {
    Regular,
    Bold,
}

/// One item assigned to a page during layout.
enum PlacedItem {
    /// A single, already wrapped line whose baseline is at `baseline_mm` and whose left edge is
    /// `left_mm` from the page edge.
    Text {
        baseline_mm: f32,
        left_mm: f32,
        text: String,
        font_size: f32,
        weight: FontWeight,
    },
    /// An inline image whose top edge is at `top_mm`.
    Image {
        top_mm: f32,
        image_index: usize,
        width_mm: f32,
        height_mm: f32,
    },
}

/// A staged non-image attachment to embed as a PDF file attachment.
struct EmbeddedAttachment {
    file_name: String,
    mime_type: String,
    file_bytes: Vec<u8>,
}

/// A contents entry: label and the 1-based page number where its section starts.
struct ContentsEntry {
    label: String,
    page_number: usize,
}

/// Accumulates body pages while tracking the current vertical position.
struct BodyLayout<'fonts> {
    /// Fonts used to measure text while wrapping.
    text_fonts: &'fonts TextFonts,
    /// Page number of the first body page (after the title and contents pages).
    first_page_number: usize,
    pages: Vec<Vec<PlacedItem>>,
    cursor_mm: f32,
    images: Vec<DynamicImage>,
    embedded_attachments: Vec<EmbeddedAttachment>,
    contents_entries: Vec<ContentsEntry>,
}

impl<'fonts> BodyLayout<'fonts> {
    /// Starts an empty layout whose first page will be `first_page_number`.
    fn new(first_page_number: usize, text_fonts: &'fonts TextFonts) -> Self {
        Self {
            text_fonts,
            first_page_number,
            pages: vec![Vec::new()],
            cursor_mm: CONTENT_TOP_MM,
            images: Vec::new(),
            embedded_attachments: Vec::new(),
            contents_entries: Vec::new(),
        }
    }

    /// Returns the absolute page number currently being filled.
    fn current_page_number(&self) -> usize {
        self.first_page_number + self.pages.len() - 1
    }

    /// Reports whether the current page has no content yet.
    fn current_page_is_empty(&self) -> bool {
        self.pages.last().is_none_or(Vec::is_empty)
    }

    /// Starts a new page unless the current one is still empty.
    fn break_page(&mut self) {
        if !self.current_page_is_empty() {
            self.pages.push(Vec::new());
        }
        self.cursor_mm = CONTENT_TOP_MM;
    }

    /// Breaks the page when fewer than `required_height_mm` remain above the bottom margin.
    fn ensure_space(&mut self, required_height_mm: f32) {
        if self.cursor_mm - required_height_mm < CONTENT_BOTTOM_MM {
            self.break_page();
        }
    }

    /// Adds an item to the current page.
    fn place(&mut self, item: PlacedItem) {
        if let Some(current_page) = self.pages.last_mut() {
            current_page.push(item);
        }
    }

    /// Wraps `text` and places each line, breaking pages as needed.
    fn push_paragraph(&mut self, text: &str, font_size: f32, weight: FontWeight) {
        let line_height_mm = line_height(font_size);
        for wrapped_line in wrap_text(text, font_size, weight, self.text_fonts) {
            self.ensure_space(line_height_mm);
            self.place(PlacedItem::Text {
                baseline_mm: self.cursor_mm,
                left_mm: LEFT_MARGIN_MM,
                text: wrapped_line,
                font_size,
                weight,
            });
            self.cursor_mm -= line_height_mm;
        }
    }

    /// Places a section heading, keeping it on the same page as at least one following line, and
    /// records it as a contents entry.
    fn push_heading(&mut self, heading_text: &str, contents_label: String, font_size: f32) {
        self.ensure_space(line_height(font_size) + 2.0 * line_height(BODY_FONT_SIZE));
        self.contents_entries.push(ContentsEntry {
            label: contents_label,
            page_number: self.current_page_number(),
        });
        self.cursor_mm -= 2.0;
        self.push_paragraph(heading_text, font_size, FontWeight::Bold);
        self.cursor_mm -= 2.0;
    }

    /// Places one block of a flowing document (#130): its gap, then its wrapped lines, the first
    /// after its marker and the rest aligned after the marker.
    fn push_flow_block(&mut self, block: &FlowBlock) {
        let line_height_mm = line_height(block.font_size);
        if !self.current_page_is_empty() {
            self.cursor_mm -= block.space_before_mm;
        }
        let marker_text = block
            .marker
            .as_ref()
            .map(|marker| format!("{marker} "))
            .unwrap_or_default();
        let marker_width_mm =
            self.text_fonts
                .text_width_mm(&marker_text, block.font_size, block.weight);
        let text_indent_mm = block.indent_mm + marker_width_mm;
        let wrapped_lines = wrap_text_to_width(
            &block.text,
            block.font_size,
            block.weight,
            self.text_fonts,
            (TEXT_WIDTH_MM - text_indent_mm).max(TEXT_WIDTH_MM / 4.0),
        );
        // A heading stays on the page of the lines that follow it.
        let first_needs_mm = if block.keep_with_next {
            line_height_mm + 2.0 * line_height(BODY_FONT_SIZE)
        } else {
            line_height_mm
        };
        self.ensure_space(first_needs_mm);
        for (line_index, wrapped_line) in wrapped_lines.into_iter().enumerate() {
            self.ensure_space(line_height_mm);
            let (text, left_mm) = if line_index == 0 && !marker_text.is_empty() {
                (
                    format!("{marker_text}{wrapped_line}"),
                    LEFT_MARGIN_MM + block.indent_mm,
                )
            } else {
                (wrapped_line, LEFT_MARGIN_MM + text_indent_mm)
            };
            self.place(PlacedItem::Text {
                baseline_mm: self.cursor_mm,
                left_mm,
                text,
                font_size: block.font_size,
                weight: block.weight,
            });
            self.cursor_mm -= line_height_mm;
        }
    }

    /// Adds vertical whitespace.
    fn push_gap(&mut self, gap_mm: f32) {
        self.cursor_mm -= gap_mm;
    }

    /// Fits `decoded_image` to the printable area and places it, on a new page if necessary.
    fn push_image(&mut self, decoded_image: DynamicImage) {
        let (pixel_width, pixel_height) = decoded_image.dimensions();
        let natural_width_mm = pixel_width as f32 / IMAGE_NATURAL_DPI * 25.4;
        let mut width_mm = natural_width_mm.min(TEXT_WIDTH_MM);
        let mut height_mm = width_mm * pixel_height as f32 / pixel_width.max(1) as f32;
        if height_mm > MAX_IMAGE_HEIGHT_MM {
            width_mm *= MAX_IMAGE_HEIGHT_MM / height_mm;
            height_mm = MAX_IMAGE_HEIGHT_MM;
        }
        self.ensure_space(height_mm + 2.0);
        let image_index = self.images.len();
        self.images.push(decoded_image);
        self.place(PlacedItem::Image {
            top_mm: self.cursor_mm,
            image_index,
            width_mm,
            height_mm,
        });
        self.cursor_mm -= height_mm + line_height(BODY_FONT_SIZE);
    }
}

/// Lays out and draws the complete PDF.
///
/// Inputs: Non-empty `documents`, export `options`, and where to find fonts.
/// Outputs: Serialized PDF bytes or `ExportError::Rendering`.
/// Side effects: Reads staged attachment files and, only when the text needs them, fallback font
/// files. Logs (without content) how many characters no font could draw.
pub(crate) fn render_pdf(
    documents: &[ExportDocument],
    options: &ExportOptions,
    font_config: &PdfFontConfig,
) -> Result<Vec<u8>, ExportError> {
    let text_fonts = TextFonts::load(font_config);
    let is_batch = documents.len() > 1;
    let contents_page_count = estimated_contents_entries(documents, is_batch)
        .div_ceil(CONTENTS_ENTRIES_PER_PAGE)
        .max(1);
    let mut body_layout = BodyLayout::new(2 + contents_page_count, &text_fonts);
    for (document_index, document) in documents.iter().enumerate() {
        if document_index > 0 {
            body_layout.break_page();
        }
        layout_meeting(&mut body_layout, document, is_batch);
    }
    let total_pages = 1 + contents_page_count + body_layout.pages.len();
    // All text is laid out (and so measured) before any font is registered, which is what tells
    // us which fallback fonts are needed at all.
    let title_page_items = layout_title_page(documents, options, &text_fonts);
    let contents_pages = body_layout
        .contents_entries
        .chunks(CONTENTS_ENTRIES_PER_PAGE)
        .chain(std::iter::once::<&[ContentsEntry]>(&[]))
        .take(contents_page_count)
        .enumerate()
        .map(|(contents_page_offset, contents_chunk)| {
            layout_contents_page(contents_chunk, contents_page_offset == 0, &text_fonts)
        })
        .collect::<Vec<_>>();

    let document_title = match documents {
        [single_document] => single_document.title(),
        _ => "Meeting export",
    };
    let (pdf_document, title_page_index, title_layer_index) = PdfDocument::new(
        document_title,
        Mm(PAGE_WIDTH_MM),
        Mm(PAGE_HEIGHT_MM),
        "Title page",
    );
    let pdf_fonts = register_fonts(&pdf_document, &text_fonts)?;
    let page_fonts = PageFonts {
        text_fonts: &text_fonts,
        pdf_fonts: &pdf_fonts,
    };

    let title_layer = pdf_document
        .get_page(title_page_index)
        .get_layer(title_layer_index);
    draw_page(
        &title_layer,
        &title_page_items,
        &[],
        &page_fonts,
        1,
        total_pages,
    );

    for (contents_page_offset, contents_items) in contents_pages.iter().enumerate() {
        let (page_index, layer_index) =
            pdf_document.add_page(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Contents");
        let contents_layer = pdf_document.get_page(page_index).get_layer(layer_index);
        draw_page(
            &contents_layer,
            contents_items,
            &[],
            &page_fonts,
            2 + contents_page_offset,
            total_pages,
        );
    }

    for (body_page_offset, body_page_items) in body_layout.pages.iter().enumerate() {
        let (page_index, layer_index) =
            pdf_document.add_page(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Body");
        let body_layer = pdf_document.get_page(page_index).get_layer(layer_index);
        draw_page(
            &body_layer,
            body_page_items,
            &body_layout.images,
            &page_fonts,
            body_layout.first_page_number + body_page_offset,
            total_pages,
        );
    }

    save_pdf(
        pdf_document,
        &text_fonts,
        &body_layout.embedded_attachments,
        document_title,
    )
}

/// Serializes a drawn document and applies the post-processing pass.
///
/// Inputs: The drawn document, the fonts its text was measured with, attachments to embed, and
/// the title. Outputs: The PDF bytes or `ExportError::Rendering`.
/// Side effects: Logs a count when some characters had no font.
fn save_pdf(
    pdf_document: PdfDocumentReference,
    text_fonts: &TextFonts,
    embedded_attachments: &[EmbeddedAttachment],
    document_title: &str,
) -> Result<Vec<u8>, ExportError> {
    let embedded_fonts = text_fonts.embedded();
    let uncovered_character_count =
        embedded_fonts.map_or(0, EmbeddedFonts::uncovered_character_count);
    if uncovered_character_count > 0 {
        tracing::warn!(
            uncovered_character_count,
            "PDF export drew characters that no available font covers as U+FFFD"
        );
    }
    let pdf_bytes = pdf_document
        .save_to_bytes()
        .map_err(|error| rendering_error("unable to serialize meeting PDF", &error))?;
    finalize_pdf(
        pdf_bytes,
        embedded_attachments,
        document_title,
        embedded_fonts
            .filter(|embedded_fonts| embedded_fonts.needs_unicode_map_rebuild())
            .map(EmbeddedFonts::drawn_characters)
            .as_ref(),
    )
}

/// One block of a flowing document, such as a paragraph, heading or list item of a summary.
pub(crate) struct FlowBlock {
    /// The block's words; wrapped to the page.
    pub(crate) text: String,
    /// Size in points.
    pub(crate) font_size: f32,
    /// Face.
    pub(crate) weight: FontWeight,
    /// Distance of the block from the left margin.
    pub(crate) indent_mm: f32,
    /// Drawn before the first line (`•`, `1.`); the other lines align after it.
    pub(crate) marker: Option<String>,
    /// Whitespace above the block, unless it starts a page.
    pub(crate) space_before_mm: f32,
    /// Keeps the block on the page of the next lines (headings).
    pub(crate) keep_with_next: bool,
}

/// Renders a flowing A4 document: the blocks one after another, with `Page N of M` footers and
/// no title or contents page (#130).
///
/// Inputs: The document title (PDF metadata), the blocks, and where fonts are found.
/// Outputs: The PDF bytes or `ExportError::Rendering`.
/// Side effects: May read fallback font files, like `render_pdf`.
pub(crate) fn render_flow_pdf(
    document_title: &str,
    blocks: &[FlowBlock],
    font_config: &PdfFontConfig,
) -> Result<Vec<u8>, ExportError> {
    let text_fonts = TextFonts::load(font_config);
    let mut body_layout = BodyLayout::new(1, &text_fonts);
    for block in blocks {
        body_layout.push_flow_block(block);
    }
    let total_pages = body_layout.pages.len();
    let (pdf_document, first_page_index, first_layer_index) = PdfDocument::new(
        document_title,
        Mm(PAGE_WIDTH_MM),
        Mm(PAGE_HEIGHT_MM),
        "Body",
    );
    let pdf_fonts = register_fonts(&pdf_document, &text_fonts)?;
    let page_fonts = PageFonts {
        text_fonts: &text_fonts,
        pdf_fonts: &pdf_fonts,
    };
    for (page_offset, page_items) in body_layout.pages.iter().enumerate() {
        let layer = if page_offset == 0 {
            pdf_document
                .get_page(first_page_index)
                .get_layer(first_layer_index)
        } else {
            let (page_index, layer_index) =
                pdf_document.add_page(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Body");
            pdf_document.get_page(page_index).get_layer(layer_index)
        };
        draw_page(
            &layer,
            page_items,
            &[],
            &page_fonts,
            page_offset + 1,
            total_pages,
        );
    }
    save_pdf(pdf_document, &text_fonts, &[], document_title)
}

/// Counts contents entries before layout so the number of contents pages is known up front.
fn estimated_contents_entries(documents: &[ExportDocument], is_batch: bool) -> usize {
    documents
        .iter()
        .map(|document| {
            let attachments_entry = usize::from(!document.attachments().is_empty());
            let meeting_entry = usize::from(is_batch);
            meeting_entry + 2 + attachments_entry
        })
        .sum()
}

/// Lays out one meeting's transcript, attachments, and statistics sections.
///
/// Inputs: The shared `body_layout`, the meeting `document`, and whether this is a batch (which
/// adds a per-meeting heading and prefixes contents labels with the meeting title).
/// Outputs: None.
/// Side effects: Reads staged attachment files into the layout.
fn layout_meeting(body_layout: &mut BodyLayout, document: &ExportDocument, is_batch: bool) {
    let label_prefix = if is_batch {
        body_layout.push_heading(
            document.title(),
            String::from(document.title()),
            MEETING_FONT_SIZE,
        );
        format!("{}: ", document.title())
    } else {
        String::new()
    };

    body_layout.push_heading(
        "Transcript",
        format!("{label_prefix}Transcript"),
        SECTION_FONT_SIZE,
    );
    if document.transcript_segments().is_empty() {
        body_layout.push_paragraph(
            "No transcript available.",
            BODY_FONT_SIZE,
            FontWeight::Regular,
        );
    }
    for transcript_segment in document.transcript_segments() {
        body_layout.ensure_space(2.0 * line_height(BODY_FONT_SIZE));
        body_layout.push_paragraph(
            &format!(
                "{}  [{}]{}",
                transcript_segment.marked_speaker(),
                format_export_offset(transcript_segment.start_time_ms()),
                if transcript_segment.confidence().overlapping_speech {
                    "  · overlapping speech"
                } else {
                    ""
                }
            ),
            BODY_FONT_SIZE,
            FontWeight::Bold,
        );
        let marked_text = transcript_segment
            .words_with_doubt()
            .into_iter()
            .map(|(word, uncertain)| {
                if uncertain {
                    format!("{word} (?)")
                } else {
                    word.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        body_layout.push_paragraph(&marked_text, BODY_FONT_SIZE, FontWeight::Regular);
        body_layout.push_gap(2.0);
    }

    if !document.attachments().is_empty() {
        body_layout.push_heading(
            "Attachments",
            format!("{label_prefix}Attachments"),
            SECTION_FONT_SIZE,
        );
        for attachment in document.attachments() {
            layout_attachment(body_layout, attachment);
        }
    }

    body_layout.push_heading(
        "Summary statistics",
        format!("{label_prefix}Summary statistics"),
        SECTION_FONT_SIZE,
    );
    body_layout.push_paragraph(
        &format!("Word count: {}", document.word_count()),
        BODY_FONT_SIZE,
        FontWeight::Regular,
    );
    for (speaker, speaking_time_ms) in document.speaking_time_by_speaker() {
        body_layout.push_paragraph(
            &format!(
                "Speaking time: {speaker} - {}",
                format_export_offset(speaking_time_ms)
            ),
            BODY_FONT_SIZE,
            FontWeight::Regular,
        );
    }
}

/// Lays out one attachment: inline image, embedded-file note, or an unavailability note.
///
/// Inputs: The shared `body_layout` and the staged `attachment`.
/// Outputs: None.
/// Side effects: Reads the staged file, if any.
fn layout_attachment(body_layout: &mut BodyLayout, attachment: &ExportAttachment) {
    body_layout.push_paragraph(
        &format!("{} ({})", attachment.display_name(), attachment.mime_type()),
        BODY_FONT_SIZE,
        FontWeight::Bold,
    );
    let Some(staged_path) = attachment.staged_path() else {
        body_layout.push_paragraph(
            "(source file unavailable)",
            BODY_FONT_SIZE,
            FontWeight::Regular,
        );
        body_layout.push_gap(2.0);
        return;
    };
    if attachment.is_image() {
        match image_crate::open(staged_path) {
            Ok(decoded_image) => {
                body_layout.push_image(flatten_onto_white(decoded_image));
            }
            Err(_) => {
                tracing::warn!(attachment_id = %attachment.attachment_id(), "PDF export could not decode image attachment");
                body_layout.push_paragraph(
                    "(image could not be embedded)",
                    BODY_FONT_SIZE,
                    FontWeight::Regular,
                );
            }
        }
    } else {
        match fs::read(staged_path) {
            Ok(file_bytes) => {
                body_layout.embedded_attachments.push(EmbeddedAttachment {
                    file_name: String::from(attachment.display_name()),
                    mime_type: String::from(attachment.mime_type()),
                    file_bytes,
                });
                body_layout.push_paragraph(
                    "Embedded as a PDF file attachment.",
                    BODY_FONT_SIZE,
                    FontWeight::Regular,
                );
            }
            Err(_) => {
                tracing::warn!(attachment_id = %attachment.attachment_id(), "PDF export could not read staged attachment");
                body_layout.push_paragraph(
                    "(attachment could not be embedded)",
                    BODY_FONT_SIZE,
                    FontWeight::Regular,
                );
            }
        }
    }
    body_layout.push_gap(2.0);
}

/// Composites any alpha channel onto a white page background and returns an opaque RGB image.
///
/// Inputs: A decoded image of any color type.
/// Outputs: An RGB8 image; fully transparent pixels become white instead of black.
/// Side effects: None.
pub(crate) fn flatten_onto_white(decoded_image: DynamicImage) -> DynamicImage {
    if !decoded_image.color().has_alpha() {
        return DynamicImage::ImageRgb8(decoded_image.to_rgb8());
    }
    let rgba_image = decoded_image.to_rgba8();
    let flattened_image = image_crate::RgbImage::from_fn(
        rgba_image.width(),
        rgba_image.height(),
        |pixel_x, pixel_y| {
            let [red, green, blue, alpha] = rgba_image.get_pixel(pixel_x, pixel_y).0;
            let blend_channel = |channel: u8| {
                let opacity = u16::from(alpha);
                let blended = (u16::from(channel) * opacity + 255 * (255 - opacity)) / 255;
                u8::try_from(blended).unwrap_or(u8::MAX)
            };
            image_crate::Rgb([
                blend_channel(red),
                blend_channel(green),
                blend_channel(blue),
            ])
        },
    );
    DynamicImage::ImageRgb8(flattened_image)
}

/// Lays out the title page: meeting metadata for one meeting, or a meeting list for a batch.
fn layout_title_page(
    documents: &[ExportDocument],
    options: &ExportOptions,
    text_fonts: &TextFonts,
) -> Vec<PlacedItem> {
    let mut title_layout = BodyLayout::new(1, text_fonts);
    title_layout.cursor_mm = 230.0;
    match documents {
        [single_document] => {
            title_layout.push_paragraph(single_document.title(), TITLE_FONT_SIZE, FontWeight::Bold);
            title_layout.push_gap(8.0);
            if options.include_metadata() {
                let participants = single_document.participants();
                for metadata_line in [
                    format!("Date: {}", single_document.started_at().to_rfc3339()),
                    format!(
                        "Duration: {}",
                        format_export_offset(single_document.duration_ms())
                    ),
                    format!(
                        "Participants: {}",
                        if participants.is_empty() {
                            String::from("none")
                        } else {
                            participants.join(", ")
                        }
                    ),
                    format!("Meeting ID: {}", single_document.meeting_id().as_str()),
                ] {
                    title_layout.push_paragraph(&metadata_line, 12.0, FontWeight::Regular);
                }
            }
        }
        _ => {
            title_layout.push_paragraph("Meeting export", TITLE_FONT_SIZE, FontWeight::Bold);
            title_layout.push_gap(8.0);
            title_layout.push_paragraph(
                &format!("Meetings: {}", documents.len()),
                12.0,
                FontWeight::Regular,
            );
            for document in documents.iter().take(25) {
                let meeting_line = if options.include_metadata() {
                    format!(
                        "{} ({})",
                        document.title(),
                        document.started_at().to_rfc3339()
                    )
                } else {
                    String::from(document.title())
                };
                title_layout.push_paragraph(&meeting_line, BODY_FONT_SIZE, FontWeight::Regular);
            }
            if documents.len() > 25 {
                title_layout.push_paragraph(
                    &format!("... and {} more", documents.len() - 25),
                    BODY_FONT_SIZE,
                    FontWeight::Regular,
                );
            }
        }
    }
    title_layout.pages.into_iter().next().unwrap_or_default()
}

/// Lays out one contents page from its entries.
fn layout_contents_page(
    entries: &[ContentsEntry],
    is_first_page: bool,
    text_fonts: &TextFonts,
) -> Vec<PlacedItem> {
    let mut contents_layout = BodyLayout::new(2, text_fonts);
    if is_first_page {
        contents_layout.push_paragraph("Contents", 20.0, FontWeight::Bold);
        contents_layout.push_gap(6.0);
    }
    for entry in entries {
        contents_layout.push_paragraph(
            &format!("{} (page {})", entry.label, entry.page_number),
            12.0,
            FontWeight::Regular,
        );
    }
    contents_layout.pages.into_iter().next().unwrap_or_default()
}

/// The registered font handles used for drawing.
struct PdfFonts {
    regular: IndirectFontRef,
    bold: IndirectFontRef,
    /// Handles of the fallback fonts that are used, keyed by their `FontSlot::Fallback` index.
    fallbacks: BTreeMap<usize, IndirectFontRef>,
}

impl PdfFonts {
    /// Returns the handle that draws `font_slot`, if that font was registered.
    fn reference(&self, font_slot: FontSlot) -> Option<&IndirectFontRef> {
        match font_slot {
            FontSlot::Primary(FontWeight::Regular) => Some(&self.regular),
            FontSlot::Primary(FontWeight::Bold) => Some(&self.bold),
            FontSlot::Fallback(fallback_index) => self.fallbacks.get(&fallback_index),
        }
    }
}

/// Everything `draw_page` needs to turn text into font runs.
struct PageFonts<'fonts> {
    text_fonts: &'fonts TextFonts,
    pdf_fonts: &'fonts PdfFonts,
}

/// Registers the fonts the laid-out text uses with the document.
///
/// Inputs: The document and the fonts used during layout.
/// Outputs: Handles for both primary faces and every used fallback font, or
/// `ExportError::Rendering` when printpdf rejects a font that already parsed.
/// Side effects: Adds font resources to `pdf_document`; printpdf subsets them on save.
fn register_fonts(
    pdf_document: &PdfDocumentReference,
    text_fonts: &TextFonts,
) -> Result<PdfFonts, ExportError> {
    let Some(embedded_fonts) = text_fonts.embedded() else {
        return Ok(PdfFonts {
            regular: builtin_font(pdf_document, BuiltinFont::Helvetica)?,
            bold: builtin_font(pdf_document, BuiltinFont::HelveticaBold)?,
            fallbacks: BTreeMap::new(),
        });
    };
    let embed_font = |loaded_font: &LoadedFont| {
        pdf_document
            .add_external_font(loaded_font.font_bytes())
            .map_err(|error| rendering_error("unable to embed PDF font", &error))
    };
    Ok(PdfFonts {
        regular: embed_font(embedded_fonts.primary(FontWeight::Regular))?,
        bold: embed_font(embedded_fonts.primary(FontWeight::Bold))?,
        fallbacks: embedded_fonts
            .used_fallback_fonts()
            .into_iter()
            .map(|(fallback_index, fallback_font)| {
                embed_font(fallback_font).map(|font_reference| (fallback_index, font_reference))
            })
            .collect::<Result<_, _>>()?,
    })
}

/// Draws placed items and the page footer onto one layer.
fn draw_page(
    layer: &printpdf::PdfLayerReference,
    items: &[PlacedItem],
    images: &[DynamicImage],
    page_fonts: &PageFonts<'_>,
    page_number: usize,
    total_pages: usize,
) {
    for item in items {
        match item {
            PlacedItem::Text {
                baseline_mm,
                left_mm,
                text,
                font_size,
                weight,
            } => draw_text_line(
                layer,
                text,
                (*font_size, *weight),
                (*left_mm, *baseline_mm),
                page_fonts,
            ),
            PlacedItem::Image {
                top_mm,
                image_index,
                width_mm,
                height_mm,
            } => {
                if let Some(decoded_image) = images.get(*image_index) {
                    let pixel_width = decoded_image.width().max(1) as f32;
                    let rendered_dpi = 300.0_f32;
                    let natural_width_mm = pixel_width / rendered_dpi * 25.4;
                    let scale = width_mm / natural_width_mm;
                    Image::from_dynamic_image(decoded_image).add_to_layer(
                        layer.clone(),
                        ImageTransform {
                            translate_x: Some(Mm(LEFT_MARGIN_MM)),
                            translate_y: Some(Mm(top_mm - height_mm)),
                            scale_x: Some(scale),
                            scale_y: Some(scale),
                            dpi: Some(rendered_dpi),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }
    draw_text_line(
        layer,
        &format!("Page {page_number} of {total_pages}"),
        (9.0, FontWeight::Regular),
        (PAGE_WIDTH_MM - LEFT_MARGIN_MM - 25.0, FOOTER_BASELINE_MM),
        page_fonts,
    );
}

/// Draws one line as a single text object, switching fonts between runs.
///
/// Inputs: The layer, the line, its `(size in points, weight)`, the `(left, baseline)` position
/// in millimetres, and the fonts.
/// Outputs: None.
/// Side effects: Appends text operators to `layer`.
fn draw_text_line(
    layer: &printpdf::PdfLayerReference,
    text: &str,
    (font_size, weight): (f32, FontWeight),
    (left_mm, baseline_mm): (f32, f32),
    page_fonts: &PageFonts<'_>,
) {
    layer.begin_text_section();
    let mut run_left_mm = left_mm;
    for text_run in page_fonts.text_fonts.runs(text, weight) {
        if let Some(font_reference) = page_fonts.pdf_fonts.reference(text_run.font_slot) {
            layer.set_font(font_reference, font_size);
            layer.set_text_matrix(TextMatrix::Translate(
                Mm(run_left_mm).into(),
                Mm(baseline_mm).into(),
            ));
            if text_run.glyph_ids.is_empty() {
                layer.write_text(text_run.text, font_reference);
            } else {
                // Embedded fonts: emit the glyph IDs directly. `write_text` would look them up
                // again and clone the whole font (hundreds of KB) on every call.
                let glyph_bytes = text_run
                    .glyph_ids
                    .iter()
                    .flat_map(|glyph_id| glyph_id.to_be_bytes())
                    .collect();
                layer.add_operation(lopdf::content::Operation::new(
                    "Tj",
                    vec![Object::String(glyph_bytes, StringFormat::Hexadecimal)],
                ));
            }
        }
        run_left_mm += text_run.width_em * font_size * MM_PER_POINT;
    }
    layer.end_text_section();
}

/// Applies the lopdf post-processing pass when the document needs it.
///
/// Inputs: printpdf's bytes, attachments to embed, the document title, and the characters drawn
/// with embedded fonts when their `ToUnicode` maps need rebuilding (`None` otherwise, including in
/// built-in font mode).
/// Outputs: The bytes unchanged when nothing applies, otherwise the re-serialized PDF, or
/// `ExportError::Rendering`.
/// Side effects: None.
fn finalize_pdf(
    pdf_bytes: Vec<u8>,
    embedded_attachments: &[EmbeddedAttachment],
    document_title: &str,
    drawn_characters: Option<&BTreeSet<char>>,
) -> Result<Vec<u8>, ExportError> {
    if embedded_attachments.is_empty() && document_title.is_ascii() && drawn_characters.is_none() {
        return Ok(pdf_bytes);
    }
    let mut pdf_document = lopdf::Document::load_mem(&pdf_bytes)
        .map_err(|error| rendering_error("unable to reopen PDF for post-processing", &error))?;
    if !embedded_attachments.is_empty() {
        embed_attachments(&mut pdf_document, embedded_attachments)?;
    }
    if !document_title.is_ascii() {
        set_unicode_title(&mut pdf_document, document_title);
    }
    if let Some(drawn_characters) = drawn_characters {
        rebuild_unicode_maps(&mut pdf_document, drawn_characters);
    }
    let mut finalized_pdf_bytes = Vec::new();
    pdf_document
        .save_to(&mut finalized_pdf_bytes)
        .map_err(|error| rendering_error("unable to serialize post-processed PDF", &error))?;
    Ok(finalized_pdf_bytes)
}

/// Stores `document_title` in the `Info` dictionary as a UTF-16 text string; printpdf writes raw
/// UTF-8 bytes, which viewers decode as PDFDocEncoding mojibake.
///
/// Inputs: The reopened document and the title.
/// Outputs: None; a document without an `Info` dictionary is left unchanged.
/// Side effects: Mutates `pdf_document`.
fn set_unicode_title(pdf_document: &mut lopdf::Document, document_title: &str) {
    let Ok(info_id) = pdf_document
        .trailer
        .get(b"Info")
        .and_then(Object::as_reference)
    else {
        return;
    };
    if let Ok(info_dictionary) = pdf_document.get_dictionary_mut(info_id) {
        info_dictionary.set(
            "Title",
            Object::String(utf16_text_string(document_title), StringFormat::Hexadecimal),
        );
    }
}

/// Replaces printpdf's `ToUnicode` map of every embedded font with one rebuilt from the embedded
/// (subset) font program, which is what makes copied and searched text match what was written.
///
/// printpdf's map has two defects: it writes code points above U+FFFF bare (`<1f389>`) instead
/// of as UTF-16 surrogate pairs, so emoji copy as garbage; and where several code points share a
/// glyph it keeps the lowest one, so CJK `目` (U+76EE) copies as the Kangxi radical `⽬` (U+2F6C).
///
/// Inputs: The reopened document and every character drawn with embedded fonts.
/// Outputs: None; a font whose program cannot be read keeps printpdf's map.
/// Side effects: Mutates the `ToUnicode` streams of the Type0 fonts.
fn rebuild_unicode_maps(pdf_document: &mut lopdf::Document, drawn_characters: &BTreeSet<char>) {
    let rebuilt_maps = pdf_document
        .objects
        .values()
        .filter_map(|pdf_object| pdf_object.as_dict().ok())
        .filter(|dictionary| {
            dictionary.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Type0".as_slice())
        })
        .filter_map(|type0_font| {
            let to_unicode_id = type0_font
                .get(b"ToUnicode")
                .and_then(Object::as_reference)
                .ok()?;
            let font_program = type0_font_program(pdf_document, type0_font)?;
            let unicode_cmap = pdf_fonts::unicode_cmap(&font_program, drawn_characters)?;
            Some((to_unicode_id, unicode_cmap))
        })
        .collect::<Vec<(ObjectId, String)>>();
    for (to_unicode_id, unicode_cmap) in rebuilt_maps {
        if let Ok(Object::Stream(to_unicode_stream)) = pdf_document.get_object_mut(to_unicode_id) {
            to_unicode_stream.set_plain_content(unicode_cmap.into_bytes());
        }
    }
}

/// Returns the embedded TrueType program (`FontFile2`) behind a Type0 font dictionary.
fn type0_font_program(pdf_document: &lopdf::Document, type0_font: &Dictionary) -> Option<Vec<u8>> {
    let resolve = |pdf_object: &Object| {
        pdf_document
            .dereference(pdf_object)
            .ok()
            .map(|(_, resolved_object)| resolved_object.clone())
    };
    let descendant_fonts = resolve(type0_font.get(b"DescendantFonts").ok()?)?;
    let descendant_font = resolve(descendant_fonts.as_array().ok()?.first()?)?;
    let font_descriptor = resolve(
        descendant_font
            .as_dict()
            .ok()?
            .get(b"FontDescriptor")
            .ok()?,
    )?;
    let font_file = resolve(font_descriptor.as_dict().ok()?.get(b"FontFile2").ok()?)?;
    let font_stream = font_file.as_stream().ok()?;
    Some(
        font_stream
            .decompressed_content()
            .unwrap_or_else(|_| font_stream.content.clone()),
    )
}

/// Adds staged non-image attachments to the PDF's `EmbeddedFiles` name tree.
///
/// Inputs: The reopened document and the attachments to embed.
/// Outputs: `ExportError::Rendering` when the document has no catalog.
/// Side effects: Mutates `pdf_document`.
fn embed_attachments(
    pdf_document: &mut lopdf::Document,
    embedded_attachments: &[EmbeddedAttachment],
) -> Result<(), ExportError> {
    let mut named_file_specifications = Vec::new();
    for (attachment_index, embedded_attachment) in embedded_attachments.iter().enumerate() {
        let mut stream_parameters = Dictionary::new();
        stream_parameters.set(
            "Size",
            Object::Integer(
                i64::try_from(embedded_attachment.file_bytes.len()).unwrap_or(i64::MAX),
            ),
        );
        let mut stream_dictionary = Dictionary::new();
        stream_dictionary.set("Type", Object::Name(b"EmbeddedFile".to_vec()));
        stream_dictionary.set("Params", Object::Dictionary(stream_parameters));
        let embedded_stream_id = pdf_document.add_object(Stream::new(
            stream_dictionary,
            embedded_attachment.file_bytes.clone(),
        ));

        let mut embedded_file_references = Dictionary::new();
        embedded_file_references.set("F", Object::Reference(embedded_stream_id));
        embedded_file_references.set("UF", Object::Reference(embedded_stream_id));
        let ascii_file_name = sanitize_to_ascii(&embedded_attachment.file_name);
        let mut file_specification = Dictionary::new();
        file_specification.set("Type", Object::Name(b"Filespec".to_vec()));
        file_specification.set(
            "F",
            Object::String(ascii_file_name.clone().into_bytes(), StringFormat::Literal),
        );
        file_specification.set(
            "UF",
            Object::String(
                utf16_text_string(&embedded_attachment.file_name),
                StringFormat::Hexadecimal,
            ),
        );
        file_specification.set(
            "Desc",
            Object::String(
                sanitize_to_ascii(&embedded_attachment.mime_type).into_bytes(),
                StringFormat::Literal,
            ),
        );
        file_specification.set("EF", Object::Dictionary(embedded_file_references));
        let file_specification_id = pdf_document.add_object(file_specification);
        // Name-tree keys must be unique; the index prefix disambiguates repeated file names.
        named_file_specifications.push((
            format!("{attachment_index:04}-{ascii_file_name}"),
            file_specification_id,
        ));
    }
    named_file_specifications.sort_by(|left, right| left.0.cmp(&right.0));
    let name_tree_entries = named_file_specifications
        .into_iter()
        .flat_map(|(tree_key, file_specification_id)| {
            [
                Object::String(tree_key.into_bytes(), StringFormat::Literal),
                Object::Reference(file_specification_id),
            ]
        })
        .collect::<Vec<_>>();
    let mut embedded_files_tree = Dictionary::new();
    embedded_files_tree.set("Names", Object::Array(name_tree_entries));
    let mut names_dictionary = Dictionary::new();
    names_dictionary.set("EmbeddedFiles", Object::Dictionary(embedded_files_tree));
    let catalog = pdf_document
        .catalog_mut()
        .map_err(|error| rendering_error("PDF has no catalog", &error))?;
    catalog.set("Names", Object::Dictionary(names_dictionary));
    catalog.set("PageMode", Object::Name(b"UseAttachments".to_vec()));
    Ok(())
}

/// Registers one built-in font with the document.
fn builtin_font(
    pdf_document: &PdfDocumentReference,
    builtin: BuiltinFont,
) -> Result<IndirectFontRef, ExportError> {
    pdf_document
        .add_builtin_font(builtin)
        .map_err(|error| rendering_error("unable to load PDF font", &error))
}

/// Returns the baseline-to-baseline distance for `font_size`.
fn line_height(font_size: f32) -> f32 {
    font_size * MM_PER_POINT * 1.4
}

/// Greedily word-wraps `text` to the printable width by measured glyph advances, breaking words
/// wider than a line (including CJK runs, which have no spaces) between characters.
///
/// Inputs: Arbitrary text, a font size in points, the weight, and the fonts it is drawn with.
/// Outputs: At least one line (possibly empty); embedded newlines start new lines. Concatenating
/// the lines of a space-free paragraph gives back the paragraph.
/// Side effects: May load fallback fonts (see `TextFonts::runs`).
pub(crate) fn wrap_text(
    text: &str,
    font_size: f32,
    weight: FontWeight,
    text_fonts: &TextFonts,
) -> Vec<String> {
    wrap_text_to_width(text, font_size, weight, text_fonts, TEXT_WIDTH_MM)
}

/// Wraps like `wrap_text`, to `width_mm` instead of the full printable width.
fn wrap_text_to_width(
    text: &str,
    font_size: f32,
    weight: FontWeight,
    text_fonts: &TextFonts,
    width_mm: f32,
) -> Vec<String> {
    let space_width_mm = text_fonts.text_width_mm(" ", font_size, weight);
    let mut wrapped_lines = Vec::new();
    for source_line in text.lines() {
        let mut current_line = String::new();
        let mut current_width_mm = 0.0_f32;
        for word in source_line.split_whitespace() {
            let word_width_mm = text_fonts.text_width_mm(word, font_size, weight);
            let separator_width_mm = if current_line.is_empty() {
                0.0
            } else {
                space_width_mm
            };
            if current_width_mm + separator_width_mm + word_width_mm <= width_mm {
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(word);
                current_width_mm += separator_width_mm + word_width_mm;
                continue;
            }
            if !current_line.is_empty() {
                wrapped_lines.push(std::mem::take(&mut current_line));
                current_width_mm = 0.0;
            }
            if word_width_mm <= width_mm {
                current_line.push_str(word);
                current_width_mm = word_width_mm;
                continue;
            }
            for character in word.chars() {
                let character_width_mm = text_fonts.text_width_mm(
                    character.encode_utf8(&mut [0_u8; 4]),
                    font_size,
                    weight,
                );
                if current_width_mm + character_width_mm > width_mm && !current_line.is_empty() {
                    wrapped_lines.push(std::mem::take(&mut current_line));
                    current_width_mm = 0.0;
                }
                current_line.push(character);
                current_width_mm += character_width_mm;
            }
        }
        wrapped_lines.push(current_line);
    }
    if wrapped_lines.is_empty() {
        wrapped_lines.push(String::new());
    }
    wrapped_lines
}

/// Replaces non-printable-ASCII characters with `_` for PDF byte-string keys.
fn sanitize_to_ascii(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_ascii_graphic() || character == ' ' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

/// Encodes `text` as a PDF Unicode text string (UTF-16BE with byte-order mark).
fn utf16_text_string(text: &str) -> Vec<u8> {
    let mut encoded_text = vec![0xFE, 0xFF];
    for code_unit in text.encode_utf16() {
        encoded_text.extend_from_slice(&code_unit.to_be_bytes());
    }
    encoded_text
}

/// Wraps a library failure in a portable rendering error.
fn rendering_error(operation: &str, error: &dyn std::fmt::Display) -> ExportError {
    ExportError::Rendering {
        reason: format!("{operation}: {error}"),
    }
}
