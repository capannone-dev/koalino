//! Generates paginated A4 meeting PDFs with a title page, contents, transcript, attachments, and
//! statistics, using `printpdf` for drawing and its bundled `lopdf` for embedded files.
//!
//! Rendering is two-pass: pass one wraps text, decodes images, and assigns every item to a page so
//! the contents page and `Page N of M` footers know real page numbers; pass two draws the pages.
//! Image attachments are drawn inline; other staged attachments are embedded as PDF file
//! attachments (the viewer's attachments panel). Text uses the built-in Helvetica faces, which only
//! cover WinAnsi (Latin-1-like) characters; other characters are rendered as `?`.

use std::{fs, path::Path};

use printpdf::{
    BuiltinFont, Image, ImageTransform, IndirectFontRef, Mm, PdfDocument, PdfDocumentReference,
    image_crate::{self, DynamicImage, GenericImageView},
    lopdf::{self, Dictionary, Object, Stream, StringFormat},
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

use super::{ensure_documents_present, write_export_artifact};

/// A4 page width.
const PAGE_WIDTH_MM: f32 = 210.0;
/// A4 page height.
const PAGE_HEIGHT_MM: f32 = 297.0;
/// Left edge of all body content.
const LEFT_MARGIN_MM: f32 = 25.0;
/// Printable width between the left and right margins.
const TEXT_WIDTH_MM: f32 = 160.0;
/// Baseline of the first line on every page.
const CONTENT_TOP_MM: f32 = 272.0;
/// Lowest baseline allowed for body content; the footer sits below it.
const CONTENT_BOTTOM_MM: f32 = 25.0;
/// Baseline of the `Page N of M` footer.
const FOOTER_BASELINE_MM: f32 = 12.0;
/// Millimetres per typographic point.
const MM_PER_POINT: f32 = 0.3528;
/// Conservative average Helvetica glyph width as a fraction of the font size, used for wrapping.
const AVERAGE_GLYPH_WIDTH_EM: f32 = 0.52;
/// Largest height an inline image may occupy.
const MAX_IMAGE_HEIGHT_MM: f32 = 150.0;
/// Resolution used to derive an image's natural size before fitting it to the page.
const IMAGE_NATURAL_DPI: f32 = 150.0;
/// Contents entries that fit on one contents page.
const CONTENTS_ENTRIES_PER_PAGE: usize = 30;

/// Body text size.
const BODY_FONT_SIZE: f32 = 10.0;
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
    /// Side effects: Reads staged attachment files; creates the destination directory and file.
    /// Unreadable or undecodable attachments are noted in the PDF rather than failing the export.
    fn export_meetings(
        &self,
        documents: &[ExportDocument],
        options: &ExportOptions,
        output_path: &Path,
    ) -> Result<ExportResult, ExportError> {
        ensure_documents_present(documents.len())?;
        let pdf_bytes = render_pdf(documents, options)?;
        write_export_artifact(output_path, &pdf_bytes, ExportFormat::Pdf, documents.len())
    }
}

/// Selects one of the two built-in faces.
#[derive(Clone, Copy)]
enum FontWeight {
    Regular,
    Bold,
}

/// One item assigned to a page during layout.
enum PlacedItem {
    /// A single, already wrapped line whose baseline is at `baseline_mm`.
    Text {
        baseline_mm: f32,
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
struct BodyLayout {
    /// Page number of the first body page (after the title and contents pages).
    first_page_number: usize,
    pages: Vec<Vec<PlacedItem>>,
    cursor_mm: f32,
    images: Vec<DynamicImage>,
    embedded_attachments: Vec<EmbeddedAttachment>,
    contents_entries: Vec<ContentsEntry>,
}

impl BodyLayout {
    /// Starts an empty layout whose first page will be `first_page_number`.
    fn new(first_page_number: usize) -> Self {
        Self {
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
        for wrapped_line in wrap_text(text, font_size) {
            self.ensure_space(line_height_mm);
            self.place(PlacedItem::Text {
                baseline_mm: self.cursor_mm,
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
/// Inputs: Non-empty `documents` and export `options`.
/// Outputs: Serialized PDF bytes or `ExportError::Rendering`.
/// Side effects: Reads staged attachment files.
fn render_pdf(
    documents: &[ExportDocument],
    options: &ExportOptions,
) -> Result<Vec<u8>, ExportError> {
    let is_batch = documents.len() > 1;
    let contents_page_count = estimated_contents_entries(documents, is_batch)
        .div_ceil(CONTENTS_ENTRIES_PER_PAGE)
        .max(1);
    let mut body_layout = BodyLayout::new(2 + contents_page_count);
    for (document_index, document) in documents.iter().enumerate() {
        if document_index > 0 {
            body_layout.break_page();
        }
        layout_meeting(&mut body_layout, document, is_batch);
    }
    let total_pages = 1 + contents_page_count + body_layout.pages.len();

    let document_title = match documents {
        [single_document] => single_document.title(),
        _ => "Meeting export",
    };
    let (pdf_document, title_page_index, title_layer_index) = PdfDocument::new(
        sanitize_for_builtin_font(document_title),
        Mm(PAGE_WIDTH_MM),
        Mm(PAGE_HEIGHT_MM),
        "Title page",
    );
    let fonts = PdfFonts {
        regular: builtin_font(&pdf_document, BuiltinFont::Helvetica)?,
        bold: builtin_font(&pdf_document, BuiltinFont::HelveticaBold)?,
    };

    let title_page_items = layout_title_page(documents, options);
    let title_layer = pdf_document
        .get_page(title_page_index)
        .get_layer(title_layer_index);
    draw_page(&title_layer, &title_page_items, &[], &fonts, 1, total_pages);

    for (contents_page_offset, contents_chunk) in body_layout
        .contents_entries
        .chunks(CONTENTS_ENTRIES_PER_PAGE)
        .chain(std::iter::once::<&[ContentsEntry]>(&[]))
        .take(contents_page_count)
        .enumerate()
    {
        let (page_index, layer_index) =
            pdf_document.add_page(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), "Contents");
        let contents_layer = pdf_document.get_page(page_index).get_layer(layer_index);
        let contents_items = layout_contents_page(contents_chunk, contents_page_offset == 0);
        draw_page(
            &contents_layer,
            &contents_items,
            &[],
            &fonts,
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
            &fonts,
            body_layout.first_page_number + body_page_offset,
            total_pages,
        );
    }

    let pdf_bytes = pdf_document
        .save_to_bytes()
        .map_err(|error| rendering_error("unable to serialize meeting PDF", &error))?;
    if body_layout.embedded_attachments.is_empty() {
        Ok(pdf_bytes)
    } else {
        embed_attachments(&pdf_bytes, &body_layout.embedded_attachments)
    }
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
                "{}  [{}]",
                transcript_segment.speaker(),
                format_export_offset(transcript_segment.start_time_ms())
            ),
            BODY_FONT_SIZE,
            FontWeight::Bold,
        );
        body_layout.push_paragraph(
            transcript_segment.text(),
            BODY_FONT_SIZE,
            FontWeight::Regular,
        );
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
                body_layout.push_image(DynamicImage::ImageRgb8(decoded_image.to_rgb8()));
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

/// Lays out the title page: meeting metadata for one meeting, or a meeting list for a batch.
fn layout_title_page(documents: &[ExportDocument], options: &ExportOptions) -> Vec<PlacedItem> {
    let mut title_layout = BodyLayout::new(1);
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
fn layout_contents_page(entries: &[ContentsEntry], is_first_page: bool) -> Vec<PlacedItem> {
    let mut contents_layout = BodyLayout::new(2);
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

/// The two font handles used for drawing.
struct PdfFonts {
    regular: IndirectFontRef,
    bold: IndirectFontRef,
}

/// Draws placed items and the page footer onto one layer.
fn draw_page(
    layer: &printpdf::PdfLayerReference,
    items: &[PlacedItem],
    images: &[DynamicImage],
    fonts: &PdfFonts,
    page_number: usize,
    total_pages: usize,
) {
    for item in items {
        match item {
            PlacedItem::Text {
                baseline_mm,
                text,
                font_size,
                weight,
            } => {
                let font = match weight {
                    FontWeight::Regular => &fonts.regular,
                    FontWeight::Bold => &fonts.bold,
                };
                layer.use_text(
                    sanitize_for_builtin_font(text),
                    *font_size,
                    Mm(LEFT_MARGIN_MM),
                    Mm(*baseline_mm),
                    font,
                );
            }
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
    layer.use_text(
        format!("Page {page_number} of {total_pages}"),
        9.0,
        Mm(PAGE_WIDTH_MM - LEFT_MARGIN_MM - 25.0),
        Mm(FOOTER_BASELINE_MM),
        &fonts.regular,
    );
}

/// Adds staged non-image attachments to the PDF's `EmbeddedFiles` name tree.
///
/// Inputs: Serialized `pdf_bytes` from printpdf and the attachments to embed.
/// Outputs: Re-serialized PDF bytes or `ExportError::Rendering`.
/// Side effects: None.
fn embed_attachments(
    pdf_bytes: &[u8],
    embedded_attachments: &[EmbeddedAttachment],
) -> Result<Vec<u8>, ExportError> {
    let mut pdf_document = lopdf::Document::load_mem(pdf_bytes)
        .map_err(|error| rendering_error("unable to reopen PDF for attachments", &error))?;
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

    let mut embedded_pdf_bytes = Vec::new();
    pdf_document
        .save_to(&mut embedded_pdf_bytes)
        .map_err(|error| rendering_error("unable to serialize PDF attachments", &error))?;
    Ok(embedded_pdf_bytes)
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

/// Greedily word-wraps `text` to the printable width at `font_size`, splitting overlong words.
///
/// Inputs: Arbitrary text and a font size in points.
/// Outputs: At least one line (possibly empty); embedded newlines start new lines.
/// Side effects: None.
fn wrap_text(text: &str, font_size: f32) -> Vec<String> {
    let maximum_characters =
        ((TEXT_WIDTH_MM / (font_size * MM_PER_POINT * AVERAGE_GLYPH_WIDTH_EM)) as usize).max(8);
    let mut wrapped_lines = Vec::new();
    for source_line in text.lines() {
        let mut current_line = String::new();
        for word in source_line.split_whitespace() {
            let mut remaining_word = word;
            while remaining_word.chars().count() > maximum_characters {
                if !current_line.is_empty() {
                    wrapped_lines.push(std::mem::take(&mut current_line));
                }
                let split_index = remaining_word
                    .char_indices()
                    .nth(maximum_characters)
                    .map_or(remaining_word.len(), |(byte_index, _)| byte_index);
                wrapped_lines.push(String::from(&remaining_word[..split_index]));
                remaining_word = &remaining_word[split_index..];
            }
            let separator_length = usize::from(!current_line.is_empty());
            if current_line.chars().count() + separator_length + remaining_word.chars().count()
                > maximum_characters
            {
                wrapped_lines.push(std::mem::take(&mut current_line));
            }
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(remaining_word);
        }
        wrapped_lines.push(current_line);
    }
    if wrapped_lines.is_empty() {
        wrapped_lines.push(String::new());
    }
    wrapped_lines
}

/// Replaces characters the WinAnsi-encoded built-in fonts cannot draw with `?`.
///
/// lopdf silently drops unencodable characters, which would hide missing text; substituting a
/// visible marker keeps the loss obvious to readers.
fn sanitize_for_builtin_font(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character == '\t' {
                ' '
            } else if character.is_control()
                || lopdf::Document::encode_text(Some("WinAnsiEncoding"), &character.to_string())
                    .is_empty()
            {
                '?'
            } else {
                character
            }
        })
        .collect()
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
