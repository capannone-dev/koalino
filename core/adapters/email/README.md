<!-- Explains the e-mail adapter: how imported .eml files are read for projects (Issue #85). -->

# E-mail adapter

Projects (#85) can hold e-mails: an `.eml` file picked by the user, or shared from a mail app on
Android, is kept as a project resource. This folder reads such a file for the app.

| File | Purpose |
| --- | --- |
| `mail_parser_reader.rs` | `MailParserEmailReader`, the `EmailReader` port (`ports/email.rs`) over the pure-Rust `mail-parser` crate. |
| `mail_parser_reader_tests.rs` | Headers, text and HTML-only bodies, decoded attachments, and the size limits. |

What it produces is `domain::email::EmailMessage`: subject, senders and recipients, the sending
time, a plain-text body (HTML-only mail is turned into text, so viewers never render markup), and
the decoded attachments. Files over 50 MiB, bodies over a million characters and more than 100
attachments are bounded (`domain/email.rs`).

`AttachmentFileStore::import_project_email` (in `adapters/storage`) uses it to store the `.eml`
and add each attachment of a supported type as a resource of the same project.

The crate is pinned to 0.11 because 1.0 needs a newer Rust than the project's `rust-version`
(1.88). Nothing here touches the network or logs content, names or addresses.
