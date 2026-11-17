//! Exports a meeting's summary on its own, as PDF, plain text, Word (.docx) or Markdown (#130).
//!
//! Summaries are Markdown written by a model or the owner. They are read with `pulldown-cmark`
//! into a few simple blocks (headings, paragraphs, list items made of plain, bold and italic
//! runs), which every format then renders:
//!
//! - **Markdown:** the summary as stored, under a `#` heading with the meeting title;
//! - **plain text:** the blocks without markup, headings underlined, lists with `-` and numbers;
//! - **PDF:** an A4 document drawn by the meeting PDF exporter's layout and Unicode fonts;
//! - **Word:** a minimal WordprocessingML package (written with `zip`) with real heading styles,
//!   so Word's navigation pane and outline work, bold and italic runs, and hanging-indent lists.
//!
//! Nothing is written to disk here; callers save or send the bytes.

use std::{
    fmt,
    io::{Cursor, Write},
    str::FromStr,
};

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::domain::errors::ExportError;

use super::{
    pdf_exporter::{BODY_FONT_SIZE, FlowBlock, FontWeight, render_flow_pdf},
    pdf_fonts::PdfFontConfig,
};

/// The file formats a summary can be exported to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SummaryExportFormat {
    /// A4 PDF for sharing and printing.
    Pdf,
    /// Plain UTF-8 text.
    Text,
    /// A Word document (Office Open XML).
    Docx,
    /// The Markdown as stored.
    Markdown,
}

/// A format name no summary exporter knows.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown summary format `{0}`; expected pdf, txt, docx or md")]
pub struct UnknownSummaryFormat(pub String);

impl FromStr for SummaryExportFormat {
    type Err = UnknownSummaryFormat;

    /// Reads a format name: `pdf`; `txt`/`text`; `docx`/`doc`/`word`; `md`/`markdown`
    /// (any case, surrounding spaces ignored).
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name.trim().to_ascii_lowercase().as_str() {
            "pdf" => Ok(Self::Pdf),
            "txt" | "text" => Ok(Self::Text),
            "docx" | "doc" | "word" => Ok(Self::Docx),
            "md" | "markdown" => Ok(Self::Markdown),
            _ => Err(UnknownSummaryFormat(String::from(name))),
        }
    }
}

impl fmt::Display for SummaryExportFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.file_extension())
    }
}

impl SummaryExportFormat {
    /// The file name extension, without the dot. Side effects: None.
    pub fn file_extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Text => "txt",
            Self::Docx => "docx",
            Self::Markdown => "md",
        }
    }

    /// The media type for HTTP responses. Side effects: None.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Text => "text/plain; charset=utf-8",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Markdown => "text/markdown; charset=utf-8",
        }
    }
}

/// A stretch of text with one style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryRun {
    /// The words.
    pub text: String,
    /// Drawn bold (`**…**`, table headers).
    pub bold: bool,
    /// Drawn italic (`*…*`).
    pub italic: bool,
}

impl SummaryRun {
    /// A run without style. Side effects: None.
    pub fn plain(text: &str) -> Self {
        Self {
            text: String::from(text),
            bold: false,
            italic: false,
        }
    }

    /// A bold run. Side effects: None.
    pub fn bold(text: &str) -> Self {
        Self {
            bold: true,
            ..Self::plain(text)
        }
    }
}

/// One block of a summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SummaryBlock {
    /// A heading of `level` 1 to 6.
    Heading { level: u8, runs: Vec<SummaryRun> },
    /// A paragraph; also each table row (cells joined by ` | `) and each code line.
    Paragraph { runs: Vec<SummaryRun> },
    /// A list item `depth` lists deep (0 for a top-level list) with its marker: `•` or `3.`.
    ListItem {
        depth: usize,
        marker: String,
        runs: Vec<SummaryRun>,
    },
}

impl SummaryBlock {
    /// The block's runs. Side effects: None.
    fn runs(&self) -> &[SummaryRun] {
        match self {
            Self::Heading { runs, .. } | Self::Paragraph { runs } | Self::ListItem { runs, .. } => {
                runs
            }
        }
    }
}

/// The block being read.
enum PendingKind {
    Heading(u8),
    Paragraph,
    ListItem { depth: usize, marker: String },
}

/// Turns parser events into blocks.
#[derive(Default)]
struct BlockBuilder {
    blocks: Vec<SummaryBlock>,
    pending: Option<(PendingKind, Vec<SummaryRun>)>,
    /// The open lists, innermost last: the next number of an ordered list, `None` for bullets.
    lists: Vec<Option<u64>>,
    bold_depth: usize,
    italic_depth: usize,
    in_table_head: bool,
    in_code_block: bool,
    /// Cells seen in the current table row.
    row_cells: usize,
}

impl BlockBuilder {
    /// Ends the pending block, keeping it when it has words.
    fn flush(&mut self) {
        let Some((kind, mut runs)) = self.pending.take() else {
            return;
        };
        if let Some(first) = runs.first_mut() {
            first.text = String::from(first.text.trim_start());
        }
        if let Some(last) = runs.last_mut() {
            last.text = String::from(last.text.trim_end());
        }
        runs.retain(|run| !run.text.is_empty());
        if runs.is_empty() {
            return;
        }
        self.blocks.push(match kind {
            PendingKind::Heading(level) => SummaryBlock::Heading { level, runs },
            PendingKind::Paragraph => SummaryBlock::Paragraph { runs },
            PendingKind::ListItem { depth, marker } => SummaryBlock::ListItem {
                depth,
                marker,
                runs,
            },
        });
    }

    /// Ends the pending block and starts one of `kind`.
    fn open(&mut self, kind: PendingKind) {
        self.flush();
        self.pending = Some((kind, Vec::new()));
    }

    /// Starts a paragraph, unless it is the first paragraph of a list item (a loose list).
    fn open_paragraph(&mut self) {
        let continues_item = matches!(
            &self.pending,
            Some((PendingKind::ListItem { .. }, runs)) if runs.is_empty()
        );
        if !continues_item {
            self.open(PendingKind::Paragraph);
        }
    }

    /// Starts a list item with the innermost list's next marker.
    fn open_item(&mut self) {
        let depth = self.lists.len().saturating_sub(1);
        let marker = match self.lists.last_mut() {
            Some(Some(next_number)) => {
                let marker = format!("{next_number}.");
                *next_number += 1;
                marker
            }
            _ => String::from("•"),
        };
        self.open(PendingKind::ListItem { depth, marker });
    }

    /// Adds words in the current style, to a new paragraph when no block is open.
    fn push_text(&mut self, text: &str) {
        if self.pending.is_none() {
            self.pending = Some((PendingKind::Paragraph, Vec::new()));
        }
        let bold = self.bold_depth > 0 || self.in_table_head;
        let italic = self.italic_depth > 0;
        if let Some((_, runs)) = self.pending.as_mut() {
            match runs.last_mut() {
                Some(last) if last.bold == bold && last.italic == italic => {
                    last.text.push_str(text);
                }
                _ => runs.push(SummaryRun {
                    text: String::from(text),
                    bold,
                    italic,
                }),
            }
        }
    }

    /// Takes in one parser event.
    fn take(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag_end) => self.end(tag_end),
            Event::Text(text) if self.in_code_block => {
                for line in text.lines() {
                    self.open(PendingKind::Paragraph);
                    self.push_text(line);
                }
                self.flush();
            }
            Event::Text(text) | Event::Code(text) => self.push_text(&text),
            Event::SoftBreak | Event::HardBreak => self.push_text(" "),
            _ => {}
        }
    }

    /// Handles the start of an element.
    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => self.open(PendingKind::Heading(heading_number(level))),
            Tag::Paragraph => self.open_paragraph(),
            Tag::List(first_number) => {
                self.flush();
                self.lists.push(first_number);
            }
            Tag::Item => self.open_item(),
            Tag::Strong => self.bold_depth += 1,
            Tag::Emphasis => self.italic_depth += 1,
            Tag::TableHead => {
                self.in_table_head = true;
                self.row_cells = 0;
                self.open(PendingKind::Paragraph);
            }
            Tag::TableRow => {
                self.row_cells = 0;
                self.open(PendingKind::Paragraph);
            }
            Tag::TableCell => {
                if self.row_cells > 0 {
                    self.push_text(" | ");
                }
                self.row_cells += 1;
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.in_code_block = true;
            }
            _ => {}
        }
    }

    /// Handles the end of an element.
    fn end(&mut self, tag_end: TagEnd) {
        match tag_end {
            TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::TableRow | TagEnd::Item => {
                self.flush();
            }
            TagEnd::TableHead => {
                self.flush();
                self.in_table_head = false;
            }
            TagEnd::List(_) => {
                self.flush();
                self.lists.pop();
            }
            TagEnd::Strong => self.bold_depth = self.bold_depth.saturating_sub(1),
            TagEnd::Emphasis => self.italic_depth = self.italic_depth.saturating_sub(1),
            TagEnd::CodeBlock => self.in_code_block = false,
            _ => {}
        }
    }
}

/// A heading level as a number from 1 to 6.
fn heading_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Reads a summary's Markdown into blocks.
///
/// Inputs: Any Markdown. Links keep their text, images and HTML are dropped, tables become one
/// paragraph per row. Outputs: The blocks in order, none of them empty. Side effects: None.
pub fn parse_summary_blocks(markdown: &str) -> Vec<SummaryBlock> {
    let mut builder = BlockBuilder::default();
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    for event in Parser::new_ext(markdown, options) {
        builder.take(event);
    }
    builder.flush();
    builder.blocks
}

/// The runs' words without styles. Side effects: None.
fn plain_text(runs: &[SummaryRun]) -> String {
    runs.iter().map(|run| run.text.as_str()).collect()
}

/// Renders a meeting's summary as a file.
///
/// Inputs: The format, the meeting's title (the document's title), the summary's Markdown, and
/// where the PDF fonts are found (unused by the other formats).
/// Outputs: The file's bytes, or `ExportError::Rendering` when the PDF or Word writer fails.
/// Side effects: The PDF may read fallback font files.
pub fn render_summary(
    format: SummaryExportFormat,
    meeting_title: &str,
    summary_markdown: &str,
    font_config: &PdfFontConfig,
) -> Result<Vec<u8>, ExportError> {
    let title = meeting_title.trim();
    match format {
        SummaryExportFormat::Markdown => {
            Ok(summary_as_markdown(title, summary_markdown).into_bytes())
        }
        SummaryExportFormat::Text => {
            Ok(summary_as_text(title, &parse_summary_blocks(summary_markdown)).into_bytes())
        }
        SummaryExportFormat::Pdf => render_flow_pdf(
            title,
            &summary_flow_blocks(title, &parse_summary_blocks(summary_markdown)),
            font_config,
        ),
        SummaryExportFormat::Docx => {
            summary_as_docx(title, &parse_summary_blocks(summary_markdown))
        }
    }
}

/// Names the file after the meeting: `<title> - summary.<ext>`, path and reserved characters
/// replaced by `-`, the title cut to 80 characters, `Meeting` for a blank title.
///
/// Side effects: None.
pub fn summary_file_name(meeting_title: &str, format: SummaryExportFormat) -> String {
    let cleaned: String = meeting_title
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            character if character.is_control() => '-',
            character => character,
        })
        .take(80)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    let stem = if cleaned.is_empty() {
        "Meeting"
    } else {
        cleaned
    };
    format!("{stem} - summary.{}", format.file_extension())
}

/// The Markdown export: the summary under a `#` title, ending with a newline.
fn summary_as_markdown(title: &str, summary_markdown: &str) -> String {
    let mut markdown = format!("# {title}\n\n{summary_markdown}");
    if !markdown.ends_with('\n') {
        markdown.push('\n');
    }
    markdown
}

/// Underlines `text` with `underline` characters, one per character.
fn underlined(text: &str, underline: char) -> String {
    let rule: String = std::iter::repeat_n(underline, text.chars().count()).collect();
    format!("{text}\n{rule}\n")
}

/// The plain-text export: an underlined title, underlined headings, indented list items.
fn summary_as_text(title: &str, blocks: &[SummaryBlock]) -> String {
    let mut text = underlined(title, '=');
    let mut after_list_item = false;
    for block in blocks {
        let words = plain_text(block.runs());
        match block {
            SummaryBlock::Heading { .. } => {
                text.push('\n');
                text.push_str(&underlined(&words, '-'));
                after_list_item = false;
            }
            SummaryBlock::Paragraph { .. } => {
                text.push('\n');
                text.push_str(&words);
                text.push('\n');
                after_list_item = false;
            }
            SummaryBlock::ListItem { depth, marker, .. } => {
                if !after_list_item {
                    text.push('\n');
                }
                let marker = if marker == "•" {
                    "-"
                } else {
                    marker.as_str()
                };
                text.push_str(&"  ".repeat(*depth));
                text.push_str(&format!("{marker} {words}\n"));
                after_list_item = true;
            }
        }
    }
    text
}

/// Title size in the PDF, in points.
const PDF_TITLE_FONT_SIZE: f32 = 20.0;
/// Indent of each list level in the PDF.
const PDF_LIST_INDENT_MM: f32 = 6.0;

/// The PDF's blocks: the title, then each summary block.
fn summary_flow_blocks(title: &str, blocks: &[SummaryBlock]) -> Vec<FlowBlock> {
    let mut flow_blocks = vec![FlowBlock {
        text: String::from(title),
        font_size: PDF_TITLE_FONT_SIZE,
        weight: FontWeight::Bold,
        indent_mm: 0.0,
        marker: None,
        space_before_mm: 0.0,
        keep_with_next: true,
    }];
    let mut after_list_item = false;
    for block in blocks {
        let text = plain_text(block.runs());
        flow_blocks.push(match block {
            SummaryBlock::Heading { level, .. } => {
                after_list_item = false;
                FlowBlock {
                    text,
                    font_size: match level {
                        1 => 16.0,
                        2 => 14.0,
                        _ => 12.0,
                    },
                    weight: FontWeight::Bold,
                    indent_mm: 0.0,
                    marker: None,
                    space_before_mm: 6.0,
                    keep_with_next: true,
                }
            }
            SummaryBlock::Paragraph { .. } => {
                after_list_item = false;
                FlowBlock {
                    text,
                    font_size: BODY_FONT_SIZE,
                    weight: FontWeight::Regular,
                    indent_mm: 0.0,
                    marker: None,
                    space_before_mm: 3.0,
                    keep_with_next: false,
                }
            }
            SummaryBlock::ListItem { depth, marker, .. } => {
                let space_before_mm = if after_list_item { 1.0 } else { 3.0 };
                after_list_item = true;
                FlowBlock {
                    text,
                    font_size: BODY_FONT_SIZE,
                    weight: FontWeight::Regular,
                    indent_mm: PDF_LIST_INDENT_MM * (*depth as f32 + 1.0),
                    marker: Some(marker.clone()),
                    space_before_mm,
                    keep_with_next: false,
                }
            }
        });
    }
    flow_blocks
}

/// The WordprocessingML namespace.
const WORD_NAMESPACE: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Escapes text for XML, dropping characters XML 1.0 forbids.
fn xml_escaped(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\t' | '\n' | '\r' => escaped.push(character),
            character if character.is_control() => {}
            character => escaped.push(character),
        }
    }
    escaped
}

/// A Word run.
fn docx_run(run: &SummaryRun) -> String {
    let mut properties = String::new();
    if run.bold {
        properties.push_str("<w:b/>");
    }
    if run.italic {
        properties.push_str("<w:i/>");
    }
    let properties = if properties.is_empty() {
        properties
    } else {
        format!("<w:rPr>{properties}</w:rPr>")
    };
    format!(
        r#"<w:r>{properties}<w:t xml:space="preserve">{}</w:t></w:r>"#,
        xml_escaped(&run.text)
    )
}

/// A Word paragraph with the given paragraph properties and runs.
fn docx_paragraph(properties: &str, runs: &[SummaryRun]) -> String {
    let runs: String = runs.iter().map(docx_run).collect();
    format!("<w:p><w:pPr>{properties}</w:pPr>{runs}</w:p>")
}

/// Twentieths of a point each list level is indented by (a quarter inch).
const DOCX_LIST_INDENT_TWIPS: usize = 360;

/// `word/document.xml`: the title, then each block, on A4 pages.
fn docx_document_part(title: &str, blocks: &[SummaryBlock]) -> String {
    let mut body = docx_paragraph(r#"<w:pStyle w:val="Title"/>"#, &[SummaryRun::plain(title)]);
    for block in blocks {
        body.push_str(&match block {
            SummaryBlock::Heading { level, runs } => {
                docx_paragraph(&format!(r#"<w:pStyle w:val="Heading{level}"/>"#), runs)
            }
            SummaryBlock::Paragraph { runs } => docx_paragraph("", runs),
            SummaryBlock::ListItem {
                depth,
                marker,
                runs,
            } => {
                // A hanging indent; the tab after the marker reaches the text's indent.
                let item_runs: String = runs.iter().map(docx_run).collect();
                format!(
                    r#"<w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:ind w:left="{}" w:hanging="{DOCX_LIST_INDENT_TWIPS}"/></w:pPr>{}<w:r><w:tab/></w:r>{item_runs}</w:p>"#,
                    DOCX_LIST_INDENT_TWIPS * (depth + 1),
                    docx_run(&SummaryRun::plain(marker)),
                )
            }
        });
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="{WORD_NAMESPACE}"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    )
}

/// One heading style; `level` 1 to 6.
fn docx_heading_style(level: u8) -> String {
    let half_points = match level {
        1 => 32,
        2 => 28,
        3 => 26,
        _ => 24,
    };
    format!(
        r#"<w:style w:type="paragraph" w:styleId="Heading{level}"><w:name w:val="heading {level}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:uiPriority w:val="9"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="80"/><w:outlineLvl w:val="{}"/></w:pPr><w:rPr><w:b/><w:sz w:val="{half_points}"/><w:szCs w:val="{half_points}"/></w:rPr></w:style>"#,
        level - 1
    )
}

/// `word/styles.xml`: Normal, Title, Heading 1–6 and List Paragraph.
fn docx_styles_part() -> String {
    let headings: String = (1..=6).map(docx_heading_style).collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="{WORD_NAMESPACE}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:eastAsia="Calibri" w:cs="Calibri"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:after="240"/></w:pPr><w:rPr><w:b/><w:sz w:val="44"/><w:szCs w:val="44"/></w:rPr></w:style>{headings}<w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:after="60"/></w:pPr></w:style></w:styles>"#
    )
}

/// The Word export: a zip of the package parts.
fn summary_as_docx(title: &str, blocks: &[SummaryBlock]) -> Result<Vec<u8>, ExportError> {
    let parts = [
        (
            "[Content_Types].xml",
            String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#,
            ),
        ),
        (
            "_rels/.rels",
            String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#,
            ),
        ),
        (
            "docProps/core.xml",
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{}</dc:title></cp:coreProperties>"#,
                xml_escaped(title)
            ),
        ),
        (
            "word/_rels/document.xml.rels",
            String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
            ),
        ),
        ("word/styles.xml", docx_styles_part()),
        ("word/document.xml", docx_document_part(title, blocks)),
    ];
    let zip_error = |error: &dyn fmt::Display| ExportError::Rendering {
        reason: format!("unable to write the Word document: {error}"),
    };
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (part_name, content) in parts {
        writer
            .start_file(part_name, options)
            .map_err(|error| zip_error(&error))?;
        writer
            .write_all(content.as_bytes())
            .map_err(|error| zip_error(&error))?;
    }
    writer
        .finish()
        .map(Cursor::into_inner)
        .map_err(|error| zip_error(&error))
}
