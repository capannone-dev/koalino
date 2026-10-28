<!-- This directory contains the format-specific meeting export adapters behind the MeetingExporter port. -->

# Export adapters

This directory implements `ports::export::MeetingExporter` once per output format. Exporters are
pure renderers: `application::export_manager` extracts meeting data from the repositories, stages
attachment copies, and hands each exporter fully built `ExportDocument` values. No exporter reads a
repository, and only the PDF exporter opens attachment files (the staged copies).

| File | Format | Extension |
|---|---|---|
| `markdown_exporter.rs` | CommonMark | `.md` |
| `pdf_exporter.rs` | A4 PDF | `.pdf` |
| `pdf_fonts.rs` | PDF font selection, fallback discovery, and glyph metrics (see [Fonts](#pdf-fonts)) | — |
| `json_exporter.rs` | JSON schema `1.0` | `.json` |
| `mod.rs` | `default_exporters()`, `default_export_directory()`, atomic file writing | — |
| `tests.rs` | Renderer contract tests with fixed fixtures, plus a `ToUnicode`-aware PDF text extractor | — |
| `pdf_unicode_tests.rs` | PDF Unicode, emoji, fallback, size, and speed contracts (Issue #37) | — |
| `fonts/` | Bundled DejaVu Sans Regular and Bold, with their license | — |
| `test_fonts/` | Tiny OFL CJK and emoji subset fonts used only by the tests | — |

All exporters write through `write_export_artifact`, which writes a hidden `.partial-<uuid>` sibling
and renames it over the destination, so readers never see half-written files. Every `ExportResult`
carries `schema_version` and `meeting_count` metadata. An empty batch is rejected with
`ExportError::NoMeetingsSelected`. `ExportFormat::Srt` is reserved; no exporter implements it.

## Markdown

- One meeting: `# <title>`, then `## Meeting metadata` (date, `HH:MM:SS` duration, participants;
  omitted when `include_metadata` is false), `## Transcript`, `## Attachments`, and
  `## Summary statistics` (word count and speaking time per speaker).
- Transcript lines look like `**Alice** — [00:00:30] text`. Speaker names, titles, and text are
  escaped so user content cannot inject emphasis, links, or headings.
- Attachments link to the staged copy **relative to the Markdown file** using CommonMark angle
  brackets (`[slides.pdf](<meeting_attachments/slides.pdf>)`). Images use `![…](…)` so they show
  inline. Attachments whose source file was missing are listed as `source file unavailable`.
- A batch starts with `# Meeting export`, renders each meeting one heading level lower, and
  separates meetings with `---`.

## PDF

- Embedded Unicode fonts (see [Fonts](#pdf-fonts)) on A4 pages with 25 mm margins. Layout runs in
  two passes: pass one wraps text by measured glyph widths, decodes images, and assigns everything
  to pages; pass two draws them. That is what lets the
  contents page show real page numbers (`Transcript (page 3)`) and every footer read `Page N of M`.
- Page order: title page (metadata, or a meeting list for batches), contents page(s), then
  per-meeting Transcript, Attachments, and Summary statistics sections. In a batch, each meeting
  starts on a new page.
- Image attachments are decoded from the staged copy and scaled to the text width (at most 150 mm
  tall). Other attachments are embedded as real PDF file attachments (`/EmbeddedFiles` name tree,
  visible in a viewer's attachments panel). Undecodable or unreadable files are noted in the
  document instead of failing the export.
- Text stays selectable, searchable, and copyable in any script: every embedded font carries a
  `ToUnicode` map, and the document title is stored as UTF-16.

### PDF fonts

Issue #37 replaced the built-in Helvetica faces (WinAnsi only) with embedded TrueType fonts.

- **Primary fonts:** DejaVu Sans Book and Bold 2.37, compiled into the binary from `fonts/`
  (about 1.4 MB). They cover Latin (including all of Latin-1), Greek, Cyrillic, and many symbols,
  so Western exports never read font files from disk.
- **Per-character fallback:** each character is drawn with the first font that has a glyph for it:
  the requested DejaVu weight, the other DejaVu weight, then each fallback font in priority order.
  A line such as `Hello 世界 🌍` is drawn as several runs, one per font, on the same baseline.
  Fallback fonts have one weight, so bold CJK or emoji text uses their regular glyphs.
- **Fallback lookup order** (the first font covering a character wins):
  1. every `.ttf` file in `NOTE_TAKER_PDF_FONT_DIR`, sorted by file name;
  2. every `.ttf` file in `~/.note_taker/fonts/` (`%USERPROFILE%\.note_taker\fonts\` on Windows),
     sorted by file name;
  3. well-known system fonts, when present: Droid Sans Fallback and Symbola on Linux, Arial Unicode
     and Apple Symbols on macOS, SimHei, Malgun Gothic, and Segoe UI Symbol on Windows.

  Fallback fonts are loaded only when a document contains a character DejaVu cannot draw, and
  only the fallbacks that draw something are embedded.
- **Missing coverage:** a character no font covers is drawn as `�` (U+FFFD), so the gap stays
  visible and the export still succeeds. The log records how many distinct characters were
  replaced, never the text. Invisible formatting characters (zero-width joiners, variation
  selectors) are dropped instead.
- **Unusable fonts:** unreadable, corrupt, collection (`.ttc`), CFF-outline (`.otf`), and
  larger-than-64-MiB files are skipped, and the log records their path. If the bundled font itself
  could not be parsed, the exporter falls back to built-in Helvetica with the old `?` substitution,
  so a font problem never fails an export.
- **Size and speed:** printpdf's `font_subsetting` feature embeds only the glyphs used. Figures
  below are for 300-segment transcripts in release builds:
  - A mixed Latin/CJK/emoji transcript is about 100 KB, even with the 17 MB Noto Sans SC as its
    fallback.
  - The two subset DejaVu faces add about 60 KB to a Latin-only export.
  - A Latin-only export renders in about 5 ms, against 7 ms on the old Helvetica path, because
    glyph IDs are written straight into the content stream.
  - Text that needs fallback fonts takes longer, about 35 ms with the test fonts. Most of that is
    reading the fallback fonts and rebuilding the `ToUnicode` maps.
- **Copyable text:** printpdf's own `ToUnicode` maps write code points above U+FFFF without UTF-16
  surrogates, and pick the lowest code point for glyphs several code points share (`目` would copy
  as the radical `⽬`). When a drawn character is affected, the maps are rebuilt from the embedded
  subset fonts, preferring the characters actually drawn.
- **Color emoji:** emoji render in black and white. printpdf can embed only outline fonts, not
  the bitmap or color tables of color emoji fonts.

#### Adding a custom font

1. Pick a **static TrueType** font (`.ttf` with `glyf` outlines), not a `.ttc` collection or a
   CFF-based `.otf`. Recommended free fonts:
   - **Noto Sans SC/TC/JP/KR** for CJK: from the Google Fonts download, use
     `static/NotoSansSC-Regular.ttf`.
   - **Noto Emoji** for monochrome emoji: `static/NotoEmoji-Regular.ttf`.
2. Avoid variable fonts such as `NotoSansSC[wght].ttf`. They are embedded at their default
   instance, which for Noto Sans SC is the Thin weight.
3. Copy the file into `~/.note_taker/fonts/`, or into a directory named by
   `NOTE_TAKER_PDF_FONT_DIR`. Files load in name order, so prefix a number (`10-NotoSansSC.ttf`)
   to prefer one font over another.
4. Export again. No restart or configuration is needed, because fonts are looked up for each
   export.

To change the primary font, replace the files in `fonts/` and update `pdf_fonts.rs`. The test
`bundled_fonts_share_no_glyphs_between_code_points` must still pass, because rendering skips the
shared-glyph scan for the bundled fonts.

## JSON schema `1.0`

```json
{
  "schema_version": "1.0",
  "exported_at": "2026-10-27T09:00:00+00:00",
  "meetings": [
    {
      "meeting": {
        "meeting_id": "string",
        "title": "string (omitted when include_metadata is false)",
        "started_at": "RFC 3339 (omitted when include_metadata is false)",
        "duration_ms": 0
      },
      "transcript_segments": [
        {
          "start_time_ms": 0,
          "end_time_ms": 30000,
          "speaker": { "identity_id": "uuid or null", "display_name": "Alice" },
          "text": "string"
        }
      ],
      "attachments": [
        {
          "attachment_id": "uuid",
          "file_path": "original absolute source path",
          "mime_type": "image/png",
          "original_timestamp": "RFC 3339"
        }
      ],
      "voice_identities": [{ "identity_id": "uuid", "display_name": "Alice" }],
      "statistics": { "word_count": 0, "speaking_time_ms": { "Alice": 30000 } }
    }
  ]
}
```

- `meetings` is always an array, so single and batch exports share one shape.
- Offsets are milliseconds from the meeting start. `speaker.identity_id` is `null` for
  unattributed segments.
- `voice_identities` lists only the identities referenced by the transcript. Voice embeddings and
  raw audio are never exported.
- `duration_ms` is the latest segment end offset. Titles currently equal the meeting ID because
  the repository stores no separate title.
- Breaking changes must bump `schema_version`.

## Customization

The default output directory is `NOTE_TAKER_EXPORT_DIR`, else `~/Documents/NoteTaker/exports`.
Filenames follow `{date}_{title or meeting_id}_{export timestamp}.{ext}` (see
`application/README.md`). The heading and document title come from `ExportDocument::title()`,
which is the stored meeting title or the `Meeting YYYY-MM-DD HH:MM UTC` fallback (Issue #35).
To add a format, implement `MeetingExporter` for a new unit struct here, add it to
`default_exporters()`, and add its renderer tests to `tests.rs`. PDF templates are not
customizable yet. PDF fonts are; see [Adding a custom font](#adding-a-custom-font).
