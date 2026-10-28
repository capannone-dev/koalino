//! Defines the Unicode contract of the PDF exporter (Issue #37): non-Latin scripts, emoji, and
//! mixed-script lines must render with embedded fonts and stay copyable, Latin-1 output must keep
//! working, and missing or broken fonts must degrade instead of failing the export.
//!
//! Every test pins its fonts through `PdfFontConfig::bundled()` plus an explicit fallback
//! directory, so results never depend on the fonts installed on the machine or in `~/.note_taker`.
//! The CJK and emoji fallback fonts are tiny OFL-licensed subsets in `test_fonts/`.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use printpdf::lopdf::{self, Object};

use super::{
    pdf_exporter::{self, FontWeight},
    pdf_fonts::{self, PdfFontConfig, TextFonts},
    tests::{TestDirectory, fixture_start, page_text_objects, page_texts},
};
use crate::domain::models::{
    ExportDocument, ExportFormat, ExportOptions, ExportTranscriptSegment, MeetingId,
};

/// Directory holding the Noto Sans SC and Noto Emoji subset fixtures.
fn fixture_font_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("core/adapters/export/test_fonts")
}

/// The bundled fonts plus the CJK and emoji fixtures, and nothing from the host.
fn fixture_font_config() -> PdfFontConfig {
    PdfFontConfig::bundled().with_fallback_directories(vec![fixture_font_directory()])
}

/// Builds a one-meeting document whose transcript holds one segment per entry in `lines`.
fn document_with_lines(title: &str, lines: &[&str]) -> ExportDocument {
    let segments = lines
        .iter()
        .zip(0_u64..)
        .map(|(line, segment_index)| {
            ExportTranscriptSegment::new(
                segment_index * 10_000,
                segment_index * 10_000 + 9_000,
                None,
                "Unattributed",
                *line,
            )
        })
        .collect();
    ExportDocument::new(
        MeetingId::new("unicode-meeting"),
        title,
        fixture_start(),
        segments,
        Vec::new(),
        Vec::new(),
    )
}

/// Renders `document` with `font_config` and parses the bytes, proving the PDF is valid.
fn render_with(
    document: &ExportDocument,
    font_config: &PdfFontConfig,
) -> (Vec<u8>, lopdf::Document) {
    let pdf_bytes = pdf_exporter::render_pdf(
        std::slice::from_ref(document),
        &ExportOptions::for_format(ExportFormat::Pdf),
        font_config,
    )
    .unwrap_or_else(|error| panic!("PDF rendering must succeed: {error}"));
    let pdf_document = lopdf::Document::load_mem(&pdf_bytes)
        .unwrap_or_else(|error| panic!("generated PDF must parse with lopdf: {error}"));
    (pdf_bytes, pdf_document)
}

/// Returns the transcript page's text (page 3: after the title and contents pages).
fn transcript_text(pdf_document: &lopdf::Document) -> String {
    page_texts(pdf_document)
        .get(2)
        .cloned()
        .unwrap_or_else(|| panic!("a transcript page must exist"))
}

/// Counts embedded TrueType font programs (`FontFile2` streams).
fn embedded_font_program_count(pdf_document: &lopdf::Document) -> usize {
    pdf_document
        .objects
        .values()
        .filter(|pdf_object| {
            matches!(pdf_object, Object::Dictionary(dictionary)
                if dictionary.get(b"Type").and_then(Object::as_name).ok() == Some(b"FontDescriptor".as_slice())
                    && dictionary.has(b"FontFile2"))
        })
        .count()
}

/// Reports whether any font dictionary names a built-in Helvetica face.
fn uses_builtin_helvetica(pdf_document: &lopdf::Document) -> bool {
    pdf_document.objects.values().any(|pdf_object| {
        matches!(pdf_object, Object::Dictionary(dictionary)
            if dictionary.get(b"BaseFont").and_then(Object::as_name).ok()
                .is_some_and(|base_font| base_font.starts_with(b"Helvetica")))
    })
}

/// Verifies Chinese text is drawn with an embedded CJK font and maps back to the same characters.
#[test]
fn pdf_renders_chinese_text_with_embedded_unicode_font() {
    let document = document_with_lines("Weekly planning", &["测试"]);

    let (_, pdf_document) = render_with(&document, &fixture_font_config());

    assert!(transcript_text(&pdf_document).contains("测试"));
    assert!(embedded_font_program_count(&pdf_document) >= 2);
    assert!(!uses_builtin_helvetica(&pdf_document));
}

/// Verifies an emoji outside the Basic Multilingual Plane is drawn and maps back through a
/// UTF-16 surrogate pair in the `ToUnicode` CMap.
#[test]
fn pdf_renders_emoji_with_embedded_monochrome_font() {
    let document = document_with_lines("Weekly planning", &["Launch 🎉"]);

    let (_, pdf_document) = render_with(&document, &fixture_font_config());

    assert!(transcript_text(&pdf_document).contains("Launch 🎉"));
}

/// Verifies a character whose glyph the font shares with a look-alike code point copies back as
/// the character that was written: `目` (U+76EE) must not become the Kangxi radical `⽬` (U+2F6C).
#[test]
fn pdf_maps_shared_glyphs_back_to_the_drawn_character() {
    let document = document_with_lines("Weekly planning", &["项目"]);

    let (_, pdf_document) = render_with(&document, &fixture_font_config());

    assert!(transcript_text(&pdf_document).contains("项目"));
}

/// Verifies one line mixing Latin, CJK, and emoji keeps its order and switches fonts per run.
#[test]
fn pdf_renders_mixed_script_line_with_per_character_font_fallback() {
    let document = document_with_lines("Weekly planning", &["Hello 世界 🌍"]);

    let (_, pdf_document) = render_with(&document, &fixture_font_config());
    let transcript_page_id = *pdf_document
        .get_pages()
        .get(&3)
        .unwrap_or_else(|| panic!("a transcript page must exist"));
    let mixed_line = page_text_objects(&pdf_document, transcript_page_id)
        .into_iter()
        .find(|text_object| text_object.text == "Hello 世界 🌍")
        .unwrap_or_else(|| panic!("the mixed-script line must be one text object"));

    assert_eq!(mixed_line.font_names.len(), 3);
}

/// Verifies Unicode text in the title reaches the title page, the contents page, and the PDF
/// `Info` dictionary as a UTF-16 text string.
#[test]
fn pdf_preserves_unicode_meeting_title() {
    let document = document_with_lines("测试会议 🎉", &["ok"]);

    let (_, pdf_document) = render_with(&document, &fixture_font_config());
    let texts = page_texts(&pdf_document);
    let info_title = pdf_document
        .trailer
        .get(b"Info")
        .and_then(|info_reference| pdf_document.dereference(info_reference))
        .and_then(|(_, info_object)| info_object.as_dict())
        .and_then(|info_dictionary| info_dictionary.get(b"Title"))
        .and_then(Object::as_str)
        .unwrap_or_else(|error| panic!("the Info dictionary must carry a title: {error}"));
    let utf16_title = "测试会议 🎉"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();

    assert!(texts[0].contains("测试会议 🎉"));
    assert_eq!(info_title, [&[0xFE_u8, 0xFF][..], &utf16_title].concat());
}

/// Verifies Latin-1 text renders exactly with the bundled font and no fallback fonts at all.
#[test]
fn pdf_keeps_latin1_text_exact_with_bundled_font() {
    let latin1_line = "Café naïve façade, Grüße, señor, £5 ±1 ©";
    let document = document_with_lines("Résumé", &[latin1_line]);

    let (_, pdf_document) = render_with(&document, &PdfFontConfig::bundled());
    let text = transcript_text(&pdf_document);

    assert!(text.contains(latin1_line));
    assert!(page_texts(&pdf_document)[0].contains("Résumé"));
    assert!(!text.contains('\u{FFFD}'));
    assert!(!uses_builtin_helvetica(&pdf_document));
}

/// Verifies the bundled font alone covers scripts the old Helvetica output turned into `?`.
#[test]
fn pdf_renders_cyrillic_greek_and_euro_with_bundled_font() {
    let document = document_with_lines("Weekly planning", &["Привет мир, Γειά σου, 20 €"]);

    let (_, pdf_document) = render_with(&document, &PdfFontConfig::bundled());

    assert!(transcript_text(&pdf_document).contains("Привет мир, Γειά σου, 20 €"));
}

/// Verifies characters no available font covers become a visible U+FFFD instead of vanishing or
/// failing the export, while the covered text around them still renders.
#[test]
fn pdf_marks_characters_without_any_font_as_replacement_glyphs() {
    let document = document_with_lines("Weekly planning", &["测试 ok 🎉"]);

    let (_, pdf_document) = render_with(&document, &PdfFontConfig::bundled());

    assert!(transcript_text(&pdf_document).contains("\u{FFFD}\u{FFFD} ok \u{FFFD}"));
}

/// Verifies an unusable bundled font falls back to the built-in Helvetica path, which still
/// writes a valid PDF with the historical `?` substitution.
#[test]
fn pdf_falls_back_to_builtin_helvetica_when_bundled_font_is_unusable() {
    let broken_font_config =
        fixture_font_config().with_primary_fonts(b"not a font".to_vec(), b"not a font".to_vec());
    let document = document_with_lines("Weekly planning", &["Café 测试"]);

    let (_, pdf_document) = render_with(&document, &broken_font_config);

    assert!(uses_builtin_helvetica(&pdf_document));
    assert!(transcript_text(&pdf_document).contains("Café ??"));
}

/// Verifies unreadable, corrupt, and non-TrueType files in a font directory are skipped, and a
/// valid font later in the same directory is still used.
#[test]
fn pdf_skips_corrupt_font_files_in_fallback_directory() {
    let font_directory = TestDirectory::new("pdf-fonts-corrupt");
    fs::write(font_directory.path().join("0-corrupt.ttf"), [0_u8; 512])
        .unwrap_or_else(|error| panic!("corrupt font fixture must be writable: {error}"));
    let chinese_font_bytes = fs::read(fixture_font_directory().join("NotoSansSC-TestSubset.ttf"))
        .unwrap_or_else(|error| panic!("CJK fixture font must be readable: {error}"));
    fs::write(
        font_directory.path().join("1-truncated.ttf"),
        &chinese_font_bytes[..64],
    )
    .unwrap_or_else(|error| panic!("truncated font fixture must be writable: {error}"));
    fs::write(
        font_directory.path().join("2-valid.ttf"),
        &chinese_font_bytes,
    )
    .unwrap_or_else(|error| panic!("valid font fixture must be writable: {error}"));
    let font_config =
        PdfFontConfig::bundled().with_fallback_directories(vec![font_directory.path().into()]);
    let document = document_with_lines("Weekly planning", &["测试"]);

    let (_, pdf_document) = render_with(&document, &font_config);

    assert!(transcript_text(&pdf_document).contains("测试"));
}

/// Verifies a missing fallback directory is ignored rather than failing the export.
#[test]
fn pdf_ignores_missing_fallback_directory() {
    let font_config = PdfFontConfig::bundled()
        .with_fallback_directories(vec![PathBuf::from("/nonexistent/note-taker-fonts")]);
    let document = document_with_lines("Weekly planning", &["ok"]);

    let (_, pdf_document) = render_with(&document, &font_config);

    assert!(transcript_text(&pdf_document).contains("ok"));
}

/// Verifies fallback fonts are embedded only when the document needs them, so Latin exports do not
/// carry unused CJK or emoji font programs.
#[test]
fn pdf_embeds_fallback_fonts_only_when_used() {
    let latin_document = document_with_lines("Weekly planning", &["Plain text"]);
    let chinese_document = document_with_lines("Weekly planning", &["测试"]);

    let (_, latin_pdf) = render_with(&latin_document, &fixture_font_config());
    let (_, chinese_pdf) = render_with(&chinese_document, &fixture_font_config());

    assert_eq!(embedded_font_program_count(&latin_pdf), 2);
    assert_eq!(embedded_font_program_count(&chinese_pdf), 3);
}

/// Verifies wrapping measures real glyph advances: a long CJK run, whose glyphs are twice as wide
/// as the old Latin-average estimate, still wraps inside the printable width.
#[test]
fn pdf_wraps_wide_cjk_text_by_measured_width() {
    let text_fonts = TextFonts::load(&fixture_font_config());
    let long_chinese_text = "测试世界".repeat(100);

    let wrapped_lines = pdf_exporter::wrap_text(
        &long_chinese_text,
        pdf_exporter::BODY_FONT_SIZE,
        FontWeight::Regular,
        &text_fonts,
    );

    assert!(wrapped_lines.len() >= 9);
    assert_eq!(wrapped_lines.concat(), long_chinese_text);
    for wrapped_line in &wrapped_lines {
        let line_width_mm = text_fonts.text_width_mm(
            wrapped_line,
            pdf_exporter::BODY_FONT_SIZE,
            FontWeight::Regular,
        );
        assert!(
            line_width_mm <= pdf_exporter::TEXT_WIDTH_MM,
            "line is {line_width_mm} mm wide"
        );
    }
}

/// Verifies a typical one-hour transcript with mixed scripts stays far below 5 MB, because only
/// the used glyphs of each font are embedded.
#[test]
fn pdf_with_embedded_fonts_stays_small_for_typical_transcript() {
    let typical_lines = (0..300)
        .map(|line_index| match line_index % 3 {
            0 => "We reviewed the roadmap and agreed on the next milestones for the release.",
            1 => "Café façade naïve résumé — Grüße aus München.",
            _ => "测试世界 会议记录 🎉 🌍",
        })
        .collect::<Vec<_>>();
    let document = document_with_lines("Weekly planning", &typical_lines);

    let (pdf_bytes, _) = render_with(&document, &fixture_font_config());

    assert!(
        pdf_bytes.len() < 1024 * 1024,
        "PDF is {} bytes",
        pdf_bytes.len()
    );
}

/// Verifies embedding Unicode fonts costs less than twice the built-in Helvetica path on the same
/// typical transcript (best of several runs, so scheduler noise does not decide the result).
#[test]
fn pdf_font_embedding_is_less_than_twice_as_slow_as_builtin_fonts() {
    let typical_lines = (0..300)
        .map(|line_index| {
            if line_index % 2 == 0 {
                "We reviewed the roadmap and agreed on the next milestones for the release."
            } else {
                "Café façade naïve résumé — Grüße aus München."
            }
        })
        .collect::<Vec<_>>();
    let document = document_with_lines("Weekly planning", &typical_lines);
    let builtin_font_config =
        PdfFontConfig::bundled().with_primary_fonts(b"not a font".to_vec(), b"not a font".to_vec());
    let fastest_render = |font_config: &PdfFontConfig| {
        (0..5)
            .map(|_| {
                let render_start = Instant::now();
                render_with(&document, font_config);
                render_start.elapsed()
            })
            .min()
            .unwrap_or(Duration::MAX)
    };

    let builtin_duration = fastest_render(&builtin_font_config);
    let embedded_duration = fastest_render(&PdfFontConfig::bundled());

    assert!(
        embedded_duration < builtin_duration * 2,
        "embedded fonts took {embedded_duration:?}, built-in fonts {builtin_duration:?}"
    );
}

/// Verifies the bundled fonts map every glyph from a single code point. Rendering relies on this
/// to skip their `cmap` scan, so replacing the bundled fonts with ones that share glyphs must fail
/// here rather than silently produce PDFs whose text copies back as look-alike characters.
#[test]
fn bundled_fonts_share_no_glyphs_between_code_points() {
    let bundled_fonts = pdf_fonts::parse_bundled_fonts()
        .unwrap_or_else(|reason| panic!("bundled fonts must parse: {reason}"));

    for bundled_font in &bundled_fonts {
        assert!(!bundled_font.has_shared_glyphs());
    }
}

/// Verifies font discovery order: the configured directory, then `~/.note_taker/fonts`, then the
/// platform's system font files.
#[test]
fn font_config_reads_directories_in_priority_order() {
    let font_config = PdfFontConfig::from_lookup(|variable_name| match variable_name {
        "NOTE_TAKER_PDF_FONT_DIR" => Some(String::from("/custom/fonts")),
        "HOME" => Some(String::from("/home/someone")),
        _ => None,
    });

    assert_eq!(
        font_config.fallback_directories(),
        [
            PathBuf::from("/custom/fonts"),
            PathBuf::from("/home/someone/.note_taker/fonts"),
        ]
    );
    assert!(!font_config.fallback_files().is_empty());
}

/// Verifies a blank font directory variable and a missing home directory add no directories.
#[test]
fn font_config_ignores_blank_and_missing_directories() {
    let font_config = PdfFontConfig::from_lookup(|variable_name| match variable_name {
        "NOTE_TAKER_PDF_FONT_DIR" => Some(String::from("  ")),
        _ => None,
    });

    assert!(font_config.fallback_directories().is_empty());
}
