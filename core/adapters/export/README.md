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
| `json_exporter.rs` | JSON schema `1.0` | `.json` |
| `mod.rs` | `default_exporters()`, `default_export_directory()`, atomic file writing | — |
| `tests.rs` | Renderer contract tests with fixed fixtures | — |

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

- Built-in Helvetica faces on A4 pages with 25 mm margins. Layout runs in two passes: pass one wraps
  text, decodes images, and assigns everything to pages; pass two draws them. That is what lets the
  contents page show real page numbers (`Transcript (page 3)`) and every footer read `Page N of M`.
- Page order: title page (metadata, or a meeting list for batches), contents page(s), then
  per-meeting Transcript, Attachments, and Summary statistics sections. In a batch, each meeting
  starts on a new page.
- Image attachments are decoded from the staged copy and scaled to the text width (at most 150 mm
  tall). Other attachments are embedded as real PDF file attachments (`/EmbeddedFiles` name tree,
  visible in a viewer's attachments panel). Undecodable or unreadable files are noted in the
  document instead of failing the export.
- **Limitation:** the built-in fonts cover only WinAnsi (Latin-1-like) characters. Other
  characters render as `?` so missing text stays visible. Embedding a Unicode TTF is a possible
  later customization.

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
customizable yet.
