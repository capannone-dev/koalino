<!-- Explains the documents adapter: reading the text of attached files for summaries, chat and search (Issue #128). -->

# Documents adapter

Meetings and projects hold files: PDFs, Word, PowerPoint and Excel documents, LibreOffice
documents, e-mails, text, Markdown, CSV and the like. This folder turns such a file into plain text
(#128), so the meeting's summary, the meeting and project chats and project search can use what
the files say. The hand-off to the owner's own AI app does not use it: there the files go as
files (#126).

| File | Purpose |
| --- | --- |
| `mod.rs` | Declares the module. |
| `text_extraction.rs` | `extract_text(mime, name, bytes)` and `has_text(mime, name)`. |
| `text_extraction_tests.rs` | Each supported kind, broken files, pictures and the length limit. |

How each kind is read:

- Text, Markdown, CSV, TSV, JSON, YAML and logs: as UTF-8 (by MIME type, or by extension for
  project files stored as `application/octet-stream`).
- E-mails: subject, senders and body, through `adapters/email` (`mail-parser`).
- PDF: page text through the pure-Rust `lopdf` crate, at most 500 pages, under `catch_unwind` so a
  malformed file cannot take the app down. Its extraction is plain: some PDFs lose spaces between
  words or ligatures such as "fi", which language models read through. Scanned PDFs (pictures of
  pages) have no text.
- Word (`.docx`), PowerPoint (`.pptx`, slide by slide), Excel (`.xlsx`, sheet by sheet, each row's
  cells separated by tabs: shared and inline strings, numbers and formulas' last values) and
  LibreOffice (`.odt`, `.odp`, `.ods`): these are zip archives of XML, read with `zip` (deflate
  only) and `xml-rs`. Any archive member over 32 MiB is skipped, a guard against zip bombs.

Each file gives at most 200,000 characters. Callers (`bin/meeting_summaries.rs`,
`bin/notes_chat_commands.rs`, `bin/project_commands.rs`) also cap the file size (50 MiB) and the
number of files (20 per meeting). Everything works in memory on already-decrypted bytes: nothing
here writes a clear copy to disk, logs content or touches the network. All three crates are
MIT-licensed or Apache-2.0, compatible with the GPL-3.0-or-later.
