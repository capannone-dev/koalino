//! Defines export-manager behavior against the in-memory SQLite repository and a mocked voice store.
//!
//! These integration tests exercise extraction (segments, identities, attachments, metadata),
//! format dispatch, attachment staging, filename generation, batch selection, and failure handling.

use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{Duration, TimeZone, Utc};
use uuid::Uuid;

use crate::{
    adapters::{export::default_exporters, storage::sqlite_adapter::SqliteNoteRepository},
    application::export_manager::{ExportManager, ExportWriter},
    domain::{
        errors::{ExportError, StorageError},
        models::{
            Attachment, ExportDateRange, ExportFormat, ExportOptions, MeetingId, MeetingTitle,
            SpeakerIdentity, Transcript, TranscriptSegment, VoiceEmbedding,
        },
    },
    ports::{
        storage::{AttachmentRepository, NoteRepository},
        voice::VoiceVectorDB,
    },
};

/// Identity UUID for the fixture speaker Alice.
const ALICE_IDENTITY: Uuid = Uuid::from_u128(0xA11CE);
/// Identity UUID referenced by a segment but unknown to the voice store.
const DELETED_IDENTITY: Uuid = Uuid::from_u128(0xDEAD);

/// Returns display-safe identities without storing biometric vectors.
struct NamedVoiceDatabase {
    identities: Vec<SpeakerIdentity>,
}

impl VoiceVectorDB for NamedVoiceDatabase {
    fn list_speaker_identities(&self) -> Result<Vec<SpeakerIdentity>, StorageError> {
        Ok(self.identities.clone())
    }
    fn store_embedding(&self, _embedding: VoiceEmbedding) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }
    fn search_similar(
        &self,
        _vector: &[f32],
        _top_k: usize,
    ) -> Result<Vec<(VoiceEmbedding, f32)>, StorageError> {
        Ok(Vec::new())
    }
    fn delete_identity(&self, _identity_id: Uuid) -> Result<(), StorageError> {
        Ok(())
    }
}

/// Owns an in-memory repository, a voice store, and unique temp directories for one test.
struct ExportFixture {
    repository: SqliteNoteRepository,
    voice_database: NamedVoiceDatabase,
    writer: ExportWriter,
    output_directory: PathBuf,
    source_directory: PathBuf,
}

impl ExportFixture {
    /// Creates empty storage plus unique source and output directories.
    fn new() -> Self {
        let fixture_root =
            std::env::temp_dir().join(format!("note-taker-export-manager-{}", Uuid::new_v4()));
        let source_directory = fixture_root.join("sources");
        fs::create_dir_all(&source_directory)
            .unwrap_or_else(|error| panic!("source directory must be creatable: {error}"));
        Self {
            repository: SqliteNoteRepository::open_in_memory(
                crate::application::crypto::voice_encryptor::ephemeral_cipher(),
            )
            .unwrap_or_else(|error| panic!("fixture repository must open: {error}")),
            voice_database: NamedVoiceDatabase {
                identities: vec![SpeakerIdentity::new(ALICE_IDENTITY, "Alice")],
            },
            writer: ExportWriter::new(default_exporters(), fixture_root.join("exports")),
            output_directory: fixture_root.join("exports"),
            source_directory,
        }
    }

    /// Builds a manager writing into the fixture output directory.
    fn manager(
        &self,
    ) -> ExportManager<'_, SqliteNoteRepository, SqliteNoteRepository, NamedVoiceDatabase> {
        ExportManager::new(
            &self.repository,
            &self.repository,
            &self.voice_database,
            &self.writer,
        )
    }

    /// Creates a meeting with optional segments.
    fn create_meeting(
        &self,
        meeting_identifier: &str,
        segments: &[(u64, u64, Option<Uuid>, &str)],
    ) -> MeetingId {
        let meeting_id = MeetingId::new(meeting_identifier);
        self.repository
            .create_meeting(&meeting_id, &[1])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
        let transcript_segments = segments
            .iter()
            .map(|(start_time_ms, end_time_ms, speaker_identity_id, text)| {
                TranscriptSegment::new(
                    meeting_id.clone(),
                    *start_time_ms,
                    *end_time_ms,
                    *speaker_identity_id,
                    *text,
                )
            })
            .collect::<Vec<_>>();
        if !transcript_segments.is_empty() {
            self.repository
                .save_transcript_segments(&meeting_id, &transcript_segments)
                .unwrap_or_else(|error| panic!("fixture segments must persist: {error}"));
        }
        meeting_id
    }

    /// Writes a source file and links it to `meeting_id` as an attachment.
    fn attach_file(
        &self,
        meeting_id: &MeetingId,
        file_name: &str,
        mime_type: &str,
        contents: &[u8],
    ) -> PathBuf {
        let source_path = self.source_directory.join(file_name);
        fs::write(&source_path, contents)
            .unwrap_or_else(|error| panic!("attachment source must be writable: {error}"));
        self.link_attachment(meeting_id, &source_path, mime_type);
        source_path
    }

    /// Links an attachment record whose file may or may not exist.
    fn link_attachment(&self, meeting_id: &MeetingId, source_path: &Path, mime_type: &str) {
        self.repository
            .save_attachment(&Attachment::new(
                Uuid::new_v4(),
                Some(meeting_id.clone()),
                source_path.to_string_lossy(),
                mime_type,
                Utc.with_ymd_and_hms(2026, 10, 27, 9, 0, 0)
                    .single()
                    .unwrap_or_else(|| panic!("fixed timestamp must be valid")),
                "fixture-device",
            ))
            .unwrap_or_else(|error| panic!("fixture attachment must persist: {error}"));
    }
}

impl Drop for ExportFixture {
    fn drop(&mut self) {
        if let Some(fixture_root) = self.source_directory.parent() {
            let _ = fs::remove_dir_all(fixture_root);
        }
    }
}

/// Reads and parses a JSON export result.
fn read_json(file_path: &str) -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(file_path).unwrap_or_else(|error| panic!("export must be readable: {error}")),
    )
    .unwrap_or_else(|error| panic!("export must be valid JSON: {error}"))
}

/// Extracts the `ExportError` carried by an application-level error.
fn export_error(error: &anyhow::Error) -> Option<&ExportError> {
    error.downcast_ref::<ExportError>()
}

/// Verifies stored segments are exported with identity names resolved through the voice store.
#[test]
fn exports_attributed_segments_with_identity_names() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting(
        "meeting-attributed",
        &[
            (0, 4_000, Some(ALICE_IDENTITY), "Hello team."),
            (4_000, 6_000, Some(DELETED_IDENTITY), "Former colleague."),
            (6_000, 9_000, None, "Someone unknown."),
        ],
    );

    let export_result = fixture
        .manager()
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Json),
            None,
        )
        .unwrap_or_else(|error| panic!("JSON export must succeed: {error}"));
    let parsed_json = read_json(export_result.file_path());
    let segments = &parsed_json["meetings"][0]["transcript_segments"];

    assert_eq!(segments[0]["speaker"]["display_name"], "Alice");
    assert_eq!(segments[1]["speaker"]["display_name"], "Unknown speaker");
    assert_eq!(segments[2]["speaker"]["display_name"], "Unattributed");
    assert_eq!(segments[2]["start_time_ms"], 6_000);
    assert_eq!(
        parsed_json["meetings"][0]["voice_identities"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(parsed_json["meetings"][0]["meeting"]["duration_ms"], 9_000);
}

/// Verifies a meeting with only a flat transcript exports it as one unattributed segment.
#[test]
fn falls_back_to_unattributed_flat_transcript() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("meeting-flat", &[]);
    fixture
        .repository
        .save_transcript(&Transcript::new(
            meeting_id.clone(),
            "Flat transcript text.",
        ))
        .unwrap_or_else(|error| panic!("fixture transcript must persist: {error}"));

    let export_result = fixture
        .manager()
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Markdown),
            None,
        )
        .unwrap_or_else(|error| panic!("Markdown export must succeed: {error}"));
    let written_markdown = fs::read_to_string(export_result.file_path())
        .unwrap_or_else(|error| panic!("export must be readable: {error}"));

    assert!(written_markdown.contains("**Unattributed** — [00:00:00] Flat transcript text."));
}

/// Verifies a meeting with no transcript at all still exports instead of failing.
#[test]
fn meeting_without_transcript_exports_empty_transcript() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("meeting-empty", &[]);

    let export_result = fixture
        .manager()
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Pdf),
            None,
        )
        .unwrap_or_else(|error| panic!("PDF export must succeed without transcript: {error}"));

    assert_eq!(export_result.format(), ExportFormat::Pdf);
    assert!(export_result.size_bytes() > 0);
}

/// Verifies an unknown meeting yields a typed not-found error and writes nothing.
#[test]
fn unknown_meeting_returns_not_found_error() {
    let fixture = ExportFixture::new();

    let export_failure = fixture
        .manager()
        .export_meeting(
            &MeetingId::new("missing"),
            &ExportOptions::for_format(ExportFormat::Json),
            None,
        )
        .expect_err("an unknown meeting must not export");

    assert!(matches!(
        export_error(&export_failure),
        Some(ExportError::MeetingNotFound { meeting_id }) if meeting_id == "missing"
    ));
    assert!(!fixture.output_directory.exists());
}

/// Verifies the reserved SRT format is rejected before any extraction or file creation.
#[test]
fn srt_is_rejected_as_unsupported() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("meeting-srt", &[]);

    let export_failure = fixture
        .manager()
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Srt),
            None,
        )
        .expect_err("SRT must be rejected");

    assert_eq!(
        export_error(&export_failure),
        Some(&ExportError::UnsupportedFormat {
            format: ExportFormat::Srt
        })
    );
}

/// Verifies Markdown exports copy attachments next to the file and link them relatively.
#[test]
fn markdown_export_stages_attachment_copies_beside_the_file() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("meeting-md-attachments", &[(0, 1_000, None, "Hi.")]);
    let source_path = fixture.attach_file(&meeting_id, "notes.txt", "text/plain", b"agenda notes");
    fixture.link_attachment(
        &meeting_id,
        &fixture.source_directory.join("gone.txt"),
        "text/plain",
    );

    let export_result = fixture
        .manager()
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Markdown),
            None,
        )
        .unwrap_or_else(|error| panic!("Markdown export must succeed: {error}"));
    let export_path = PathBuf::from(export_result.file_path());
    let written_markdown = fs::read_to_string(&export_path)
        .unwrap_or_else(|error| panic!("export must be readable: {error}"));
    let export_stem = export_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let staged_copy = fixture
        .output_directory
        .join(format!("{export_stem}_attachments"))
        .join("notes.txt");

    assert_eq!(fs::read(&staged_copy).ok(), Some(b"agenda notes".to_vec()));
    assert!(written_markdown.contains(&format!("(<{export_stem}_attachments/notes.txt>)")));
    assert!(written_markdown.contains("gone.txt (text/plain) — source file unavailable"));
    assert!(source_path.exists(), "export must never move the source");
}

/// Verifies PDF exports embed attachments from a temporary staging copy that is cleaned up.
#[test]
fn pdf_export_embeds_attachments_and_leaves_no_staging_files() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("meeting-pdf-attachments", &[(0, 1_000, None, "Hi.")]);
    fixture.attach_file(
        &meeting_id,
        "slides.pdf",
        "application/pdf",
        b"%PDF-1.4 fixture",
    );

    let export_result = fixture
        .manager()
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Pdf),
            None,
        )
        .unwrap_or_else(|error| panic!("PDF export must succeed: {error}"));
    let pdf_document = printpdf::lopdf::Document::load(export_result.file_path())
        .unwrap_or_else(|error| panic!("PDF must parse: {error}"));
    let output_entries = fs::read_dir(&fixture.output_directory)
        .unwrap_or_else(|error| panic!("output directory must exist: {error}"))
        .count();

    assert!(
        pdf_document
            .catalog()
            .map(|catalog| catalog.has(b"Names"))
            .unwrap_or(false)
    );
    assert_eq!(
        output_entries, 1,
        "only the PDF itself belongs in the output"
    );
}

/// Verifies disabling attachments omits them and stages nothing.
#[test]
fn excluded_attachments_are_neither_listed_nor_staged() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("meeting-no-attachments", &[(0, 1_000, None, "Hi.")]);
    fixture.attach_file(&meeting_id, "notes.txt", "text/plain", b"agenda notes");

    let export_result = fixture
        .manager()
        .export_meeting(
            &meeting_id,
            &ExportOptions::new(ExportFormat::Markdown, false, true, None),
            None,
        )
        .unwrap_or_else(|error| panic!("Markdown export must succeed: {error}"));
    let written_markdown = fs::read_to_string(export_result.file_path())
        .unwrap_or_else(|error| panic!("export must be readable: {error}"));

    assert!(!written_markdown.contains("notes.txt"));
    assert_eq!(
        fs::read_dir(&fixture.output_directory)
            .map(Iterator::count)
            .unwrap_or_default(),
        1
    );
}

/// Verifies generated filenames are sanitized, dated, and unique across repeated exports.
#[test]
fn generated_filenames_are_sanitized_and_unique() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("team/sync:1", &[(0, 1_000, None, "Hi.")]);
    let manager = fixture.manager();
    let options = ExportOptions::for_format(ExportFormat::Json);

    let first_export = manager
        .export_meeting(&meeting_id, &options, None)
        .unwrap_or_else(|error| panic!("first export must succeed: {error}"));
    let second_export = manager
        .export_meeting(&meeting_id, &options, None)
        .unwrap_or_else(|error| panic!("second export must succeed: {error}"));
    let first_name = Path::new(first_export.file_path())
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or_default()
        .to_owned();

    assert_ne!(first_export.file_path(), second_export.file_path());
    assert!(Path::new(first_export.file_path()).starts_with(&fixture.output_directory));
    assert!(first_name.starts_with(&Utc::now().format("%Y-%m-%d_team_sync_1_").to_string()));
    assert!(first_name.ends_with(".json"));
}

/// Verifies a titled meeting exports with its title as the Markdown heading, JSON title, PDF
/// document title, and generated filename stem.
#[test]
fn titled_meetings_use_their_title_for_headers_and_filenames() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("meeting-3f2a", &[(0, 1_000, None, "Hi.")]);
    fixture
        .repository
        .save_meeting_title(
            &meeting_id,
            &MeetingTitle::parse("Project Sync: Q4/plan")
                .unwrap_or_else(|error| panic!("title must parse: {error}")),
        )
        .unwrap_or_else(|error| panic!("title must persist: {error}"));
    let manager = fixture.manager();

    let markdown_export = manager
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Markdown),
            None,
        )
        .unwrap_or_else(|error| panic!("Markdown export must succeed: {error}"));
    let json_export = manager
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Json),
            None,
        )
        .unwrap_or_else(|error| panic!("JSON export must succeed: {error}"));
    let pdf_document = manager
        .extract_meeting(&meeting_id, &ExportOptions::for_format(ExportFormat::Pdf))
        .unwrap_or_else(|error| panic!("extraction must succeed: {error}"));
    let written_markdown = fs::read_to_string(markdown_export.file_path())
        .unwrap_or_else(|error| panic!("export must be readable: {error}"));
    let markdown_name = Path::new(markdown_export.file_path())
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap_or_default()
        .to_owned();

    assert!(
        written_markdown.starts_with("# Project Sync: Q4/plan\n"),
        "{written_markdown}"
    );
    assert_eq!(
        read_json(json_export.file_path())["meetings"][0]["meeting"]["title"],
        "Project Sync: Q4/plan"
    );
    assert_eq!(pdf_document.title(), "Project Sync: Q4/plan");
    assert!(
        markdown_name.starts_with(
            &Utc::now()
                .format("%Y-%m-%d_Project_Sync__Q4_plan_")
                .to_string()
        ),
        "{markdown_name}"
    );
}

/// Verifies an untitled meeting exports under its dated fallback title while its filename keeps
/// the meeting ID, which the fallback (already a date) would not distinguish.
#[test]
fn untitled_meetings_export_with_the_fallback_title() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("meeting-untitled", &[(0, 1_000, None, "Hi.")]);

    let export_result = fixture
        .manager()
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Markdown),
            None,
        )
        .unwrap_or_else(|error| panic!("Markdown export must succeed: {error}"));
    let written_markdown = fs::read_to_string(export_result.file_path())
        .unwrap_or_else(|error| panic!("export must be readable: {error}"));

    assert!(
        written_markdown.starts_with(&format!("# Meeting {}", Utc::now().format("%Y-%m-%d"))),
        "{written_markdown}"
    );
    assert!(
        export_result.file_path().contains("_meeting-untitled_"),
        "{}",
        export_result.file_path()
    );
}

/// Verifies an explicit destination path overrides the generated filename.
#[test]
fn explicit_destination_overrides_generated_path() {
    let fixture = ExportFixture::new();
    let meeting_id = fixture.create_meeting("meeting-destination", &[(0, 1_000, None, "Hi.")]);
    let destination = fixture.source_directory.join("custom").join("minutes.md");

    let export_result = fixture
        .manager()
        .export_meeting(
            &meeting_id,
            &ExportOptions::for_format(ExportFormat::Markdown),
            Some(&destination),
        )
        .unwrap_or_else(|error| panic!("export to destination must succeed: {error}"));

    assert_eq!(Path::new(export_result.file_path()), destination);
    assert!(destination.exists());
}

/// Verifies batch export writes every requested meeting into one artifact.
#[test]
fn batch_export_writes_all_requested_meetings() {
    let fixture = ExportFixture::new();
    let first_meeting = fixture.create_meeting("meeting-one", &[(0, 1_000, None, "One.")]);
    let second_meeting = fixture.create_meeting("meeting-two", &[(0, 1_000, None, "Two.")]);

    let export_result = fixture
        .manager()
        .export_meetings(
            &[first_meeting, second_meeting],
            &ExportOptions::for_format(ExportFormat::Json),
            None,
        )
        .unwrap_or_else(|error| panic!("batch export must succeed: {error}"));
    let parsed_json = read_json(export_result.file_path());

    assert_eq!(export_result.metadata_value("meeting_count"), Some("2"));
    assert_eq!(
        parsed_json["meetings"][1]["meeting"]["meeting_id"],
        "meeting-two"
    );
}

/// Verifies `export_all` filters meetings by their creation date and rejects empty selections.
#[test]
fn export_all_filters_by_date_range() {
    let fixture = ExportFixture::new();
    fixture.create_meeting("meeting-today-a", &[(0, 1_000, None, "A.")]);
    fixture.create_meeting("meeting-today-b", &[(0, 1_000, None, "B.")]);
    let manager = fixture.manager();
    let around_now = ExportDateRange::new(
        Utc::now() - Duration::hours(1),
        Utc::now() + Duration::hours(1),
    )
    .unwrap_or_else(|error| panic!("range must be valid: {error}"));
    let long_ago = ExportDateRange::new(
        Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0)
            .single()
            .unwrap_or_else(|| panic!("fixed timestamp must be valid")),
        Utc.with_ymd_and_hms(2020, 12, 31, 23, 59, 59)
            .single()
            .unwrap_or_else(|| panic!("fixed timestamp must be valid")),
    )
    .unwrap_or_else(|error| panic!("range must be valid: {error}"));

    let included_export = manager
        .export_all(
            &ExportOptions::new(ExportFormat::Json, true, true, Some(around_now)),
            None,
        )
        .unwrap_or_else(|error| panic!("in-range export must succeed: {error}"));
    let empty_selection = manager
        .export_all(
            &ExportOptions::new(ExportFormat::Json, true, true, Some(long_ago)),
            None,
        )
        .expect_err("an empty selection must be rejected");

    assert_eq!(included_export.metadata_value("meeting_count"), Some("2"));
    assert_eq!(
        export_error(&empty_selection),
        Some(&ExportError::NoMeetingsSelected)
    );
}
