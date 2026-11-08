//! Specifies that meeting text is sealed at rest (#92): transcripts, transcript segments and the
//! meeting CRDT documents are stored only as AES-256-GCM ciphertext bound to their meeting,
//! databases from before #92 are sealed when they open, and another key cannot read them.
//!
//! Raw `rusqlite` connections inspect the stored bytes directly.

use std::{path::PathBuf, sync::Arc};

use rusqlite::{Connection, params};
use uuid::Uuid;

use super::sqlite_adapter::SqliteNoteRepository;
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::models::{DeviceId, MeetingId, Transcript, TranscriptSegment},
    ports::{crypto::EmbeddingCipher, storage::NoteRepository},
};

/// Words that appear in the fixture's text and must never be found on disk.
const SECRET_WORDS: [&str; 3] = ["Riservato", "acquisizione", "Zanzibar"];

fn meeting() -> MeetingId {
    MeetingId::new("meeting-secret")
}

fn cipher(key: u8) -> Arc<dyn EmbeddingCipher> {
    Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([key; 32])))
}

fn open(
    path: &PathBuf,
    key: u8,
) -> Result<SqliteNoteRepository, crate::domain::errors::StorageError> {
    SqliteNoteRepository::open(path, DeviceId::new("laptop"), cipher(key)).and_then(|repository| {
        repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
    })
}

fn scratch_database() -> PathBuf {
    let directory = std::env::temp_dir().join(format!("note-encryption-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap_or_else(|error| panic!("{error}"));
    directory.join("note_taker.sqlite3")
}

fn segments() -> Vec<TranscriptSegment> {
    vec![
        TranscriptSegment::new(meeting(), 0, 1_000, None, "Riservato: the acquisizione"),
        TranscriptSegment::new(meeting(), 1_000, 2_000, None, "closes in Zanzibar."),
    ]
}

/// Records the fixture meeting with a transcript and segments.
fn record(repository: &SqliteNoteRepository) {
    repository
        .create_meeting(&meeting(), &automerge::AutoCommit::new().save())
        .and_then(|()| {
            repository.save_transcript(&Transcript::new(
                meeting(),
                "Riservato: the acquisizione closes in Zanzibar.",
            ))
        })
        .and_then(|()| repository.save_transcript_segments(&meeting(), &segments()))
        .unwrap_or_else(|error| panic!("{error}"));
}

/// Reads every stored byte of the meeting's text columns and document.
fn stored_bytes(path: &PathBuf) -> Vec<u8> {
    let connection = Connection::open(path).unwrap_or_else(|error| panic!("{error}"));
    let (mut bytes, document): (Vec<u8>, Vec<u8>) = connection
        .query_row(
            "SELECT CAST(transcript AS BLOB), automerge_document FROM meetings",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    bytes.extend(document);
    let mut statement = connection
        .prepare("SELECT CAST(segment_text AS BLOB) FROM transcript_segments")
        .unwrap_or_else(|error| panic!("{error}"));
    for segment in statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))
        .unwrap_or_else(|error| panic!("{error}"))
    {
        bytes.extend(segment.unwrap_or_else(|error| panic!("{error}")));
    }
    bytes
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

#[test]
fn transcripts_segments_and_documents_are_stored_only_sealed() {
    let path = scratch_database();
    let repository = open(&path, 7).unwrap_or_else(|error| panic!("{error}"));
    record(&repository);

    let stored = stored_bytes(&path);
    for word in SECRET_WORDS {
        assert!(!contains(&stored, word), "{word} is on disk in clear");
    }
    assert_eq!(
        repository
            .load_transcript(&meeting())
            .ok()
            .flatten()
            .map(|transcript| transcript.content().to_owned()),
        Some("Riservato: the acquisizione closes in Zanzibar.".to_owned())
    );
    let loaded = repository
        .load_transcript_segments(&meeting())
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(loaded, segments());
    drop(repository);

    let reopened = open(&path, 7).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        reopened.load_transcript_segments(&meeting()).ok(),
        Some(segments())
    );
}

#[test]
fn a_database_from_before_sealing_is_sealed_when_it_opens() {
    let path = scratch_database();
    let repository = open(&path, 7).unwrap_or_else(|error| panic!("{error}"));
    record(&repository);
    let document = repository
        .load_crdt_document(&meeting())
        .unwrap_or_else(|error| panic!("{error}"));
    drop(repository);
    // Writes the rows back in clear, as a build before #92 stored them.
    let connection = Connection::open(&path).unwrap_or_else(|error| panic!("{error}"));
    connection
        .execute(
            "UPDATE meetings SET transcript = ?1, automerge_document = ?2",
            params!["Riservato: the acquisizione closes in Zanzibar.", document],
        )
        .and_then(|_| {
            connection.execute(
                "UPDATE transcript_segments SET segment_text = 'Riservato Zanzibar acquisizione'",
                [],
            )
        })
        .and_then(|_| {
            connection.execute("DELETE FROM storage_migrations WHERE name LIKE 'seal%'", [])
        })
        .unwrap_or_else(|error| panic!("{error}"));
    drop(connection);

    let reopened = open(&path, 7).unwrap_or_else(|error| panic!("{error}"));

    let stored = stored_bytes(&path);
    for word in SECRET_WORDS {
        assert!(!contains(&stored, word), "{word} stayed in clear");
    }
    assert_eq!(
        reopened
            .load_transcript(&meeting())
            .ok()
            .flatten()
            .map(|transcript| transcript.content().to_owned()),
        Some("Riservato: the acquisizione closes in Zanzibar.".to_owned())
    );
    assert_eq!(
        reopened
            .load_crdt_document(&meeting())
            .unwrap_or_else(|error| panic!("{error}")),
        document,
        "the document opens to the same bytes"
    );
}

#[test]
fn another_key_cannot_read_the_text() {
    let path = scratch_database();
    record(&open(&path, 7).unwrap_or_else(|error| panic!("{error}")));

    let other = SqliteNoteRepository::open(&path, DeviceId::new("laptop"), cipher(8))
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(other.load_transcript_segments(&meeting()).is_err());
    assert!(other.load_transcript(&meeting()).is_err());
}

#[test]
fn sealed_text_is_bound_to_its_meeting() {
    let path = scratch_database();
    let repository = open(&path, 7).unwrap_or_else(|error| panic!("{error}"));
    record(&repository);
    let other_meeting = MeetingId::new("meeting-other");
    repository
        .create_meeting(&other_meeting, &automerge::AutoCommit::new().save())
        .unwrap_or_else(|error| panic!("{error}"));
    // Moves the secret meeting's sealed transcript onto the other meeting's row.
    let connection = Connection::open(&path).unwrap_or_else(|error| panic!("{error}"));
    connection
        .execute(
            "UPDATE meetings SET transcript = (SELECT transcript FROM meetings WHERE meeting_id = ?1) WHERE meeting_id = ?2",
            params![meeting().as_str(), other_meeting.as_str()],
        )
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(repository.load_transcript(&other_meeting).is_err());
}
