//! Specifies reading imported e-mails: headers, the text body (also from HTML-only mail), decoded
//! attachments, and the limits that bound a file the user picked.

use chrono::{TimeZone, Utc};

use super::mail_parser_reader::MailParserEmailReader;
use crate::{
    domain::email::{EmailPerson, MAX_EMAIL_ATTACHMENTS},
    ports::email::{EmailReadError, EmailReader},
};

/// Reads `raw` with the default limits.
fn read(raw: &str) -> crate::domain::email::EmailMessage {
    MailParserEmailReader::new()
        .read_email(raw.as_bytes())
        .unwrap_or_else(|error| panic!("the e-mail must be read: {error}"))
}

/// A person with a name and an address.
fn person(name: &str, address: &str) -> EmailPerson {
    EmailPerson {
        name: Some(name.to_owned()),
        address: Some(address.to_owned()),
    }
}

const PLAIN: &str = "From: Giacomo Rossi <giacomo@example.com>\r
To: Ettore <ettore@example.com>, aaron@example.com\r
Cc: Team <team@example.com>\r
Subject: =?UTF-8?Q?Budget_2027_=E2=80=93_draft?=\r
Date: Tue, 3 Nov 2026 10:15:00 +0100\r
Content-Type: text/plain; charset=utf-8\r
\r
Hi all,\r
the draft is attached.\r
";

#[test]
fn headers_and_a_plain_body_are_read() {
    let email = read(PLAIN);

    assert_eq!(email.subject.as_deref(), Some("Budget 2027 – draft"));
    assert_eq!(
        email.from,
        vec![person("Giacomo Rossi", "giacomo@example.com")]
    );
    assert_eq!(
        email.to,
        vec![
            person("Ettore", "ettore@example.com"),
            EmailPerson {
                name: None,
                address: Some("aaron@example.com".to_owned())
            }
        ]
    );
    assert_eq!(email.cc, vec![person("Team", "team@example.com")]);
    assert_eq!(
        email.sent_at,
        Utc.with_ymd_and_hms(2026, 11, 3, 9, 15, 0).single()
    );
    assert_eq!(
        email.body_text.trim_end(),
        "Hi all,\r\nthe draft is attached."
    );
    assert!(!email.body_truncated);
    assert!(email.attachments.is_empty());
}

#[test]
fn the_text_part_is_preferred_and_html_only_mail_becomes_text() {
    let alternative = "From: a@example.com\r
Subject: Both\r
Content-Type: multipart/alternative; boundary=\"b\"\r
\r
--b\r
Content-Type: text/plain\r
\r
Plain version\r
--b\r
Content-Type: text/html\r
\r
<p>HTML <b>version</b></p>\r
--b--\r
";
    let html_only = "From: a@example.com\r
Subject: HTML\r
Content-Type: text/html\r
\r
<html><body><p>Hello <b>there</b></p><script>alert(1)</script></body></html>\r
";

    assert_eq!(read(alternative).body_text.trim(), "Plain version");
    let html_text = read(html_only).body_text;
    assert!(html_text.contains("Hello there"), "{html_text}");
    assert!(!html_text.contains('<'), "{html_text}");
}

#[test]
fn attachments_are_decoded_with_their_names_and_types() {
    let mixed = "From: a@example.com\r
Subject: Files\r
Content-Type: multipart/mixed; boundary=\"m\"\r
\r
--m\r
Content-Type: text/plain\r
\r
See attached.\r
--m\r
Content-Type: application/PDF; name=\"budget.pdf\"\r
Content-Disposition: attachment; filename=\"budget.pdf\"\r
Content-Transfer-Encoding: base64\r
\r
JVBERi0xLjQK\r
--m\r
Content-Type: image/png\r
Content-Disposition: attachment\r
Content-Transfer-Encoding: base64\r
\r
iVBORw0K\r
--m--\r
";

    let email = read(mixed);

    assert_eq!(email.body_text.trim(), "See attached.");
    assert_eq!(email.attachments.len(), 2);
    assert_eq!(email.attachments[0].file_name, "budget.pdf");
    assert_eq!(email.attachments[0].mime_type, "application/pdf");
    assert_eq!(email.attachments[0].contents, b"%PDF-1.4\n");
    assert_eq!(email.attachments[1].file_name, "attachment-2");
    assert_eq!(email.attachments[1].mime_type, "image/png");
    assert_eq!(email.attachments[1].contents, b"\x89PNG\r\n");
}

#[test]
fn a_long_body_and_many_attachments_are_bounded() {
    let mut many = String::from(
        "From: a@example.com\r\nContent-Type: multipart/mixed; boundary=\"m\"\r\n\r\n",
    );
    for index in 0..=MAX_EMAIL_ATTACHMENTS {
        many.push_str(&format!(
            "--m\r\nContent-Type: text/plain\r\nContent-Disposition: attachment; filename=\"{index}.txt\"\r\n\r\nx\r\n"
        ));
    }
    many.push_str("--m--\r\n");
    let reader = MailParserEmailReader::with_limits(1_000_000, 10);
    let long = format!(
        "From: a@example.com\r\nContent-Type: text/plain\r\n\r\n{}\r\n",
        "é".repeat(30)
    );

    assert_eq!(read(&many).attachments.len(), MAX_EMAIL_ATTACHMENTS);
    let cut = reader
        .read_email(long.as_bytes())
        .unwrap_or_else(|error| panic!("the e-mail must be read: {error}"));
    assert_eq!(cut.body_text, "é".repeat(10));
    assert!(cut.body_truncated);
}

#[test]
fn an_oversized_or_empty_file_is_refused() {
    let small = MailParserEmailReader::with_limits(16, 100);

    assert_eq!(
        small.read_email(PLAIN.as_bytes()),
        Err(EmailReadError::TooLarge)
    );
    assert_eq!(
        MailParserEmailReader::new().read_email(b""),
        Err(EmailReadError::Malformed)
    );
}
