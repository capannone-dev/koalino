//! Specifies the notes folder (#98): the tree it writes, edits read back into meetings and
//! projects, files dropped in imported, and deletions and renames that never reach the store.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::{
    notes_folder::{FolderState, NotesFolderSettings, run_round, safe_name},
    project_commands::ProjectFiles,
};
use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
        meeting_edits::{edit_summary, load_notes, set_notes},
        meeting_summary_document::read_summary,
    },
    domain::{
        models::{DeviceId, MeetingId, MeetingTitle, TranscriptSegment},
        projects::{ProjectId, ProjectName},
    },
    ports::{
        projects::ProjectRepository,
        storage::{AttachmentRepository, NoteRepository},
    },
};

/// A database, its attachment folder and a notes folder, in a scratch directory.
struct Fixture {
    scratch: PathBuf,
    repository: SqliteNoteRepository,
    files: ProjectFiles,
    root: PathBuf,
    state: FolderState,
}

impl Fixture {
    fn new() -> Self {
        let scratch = std::env::temp_dir().join(format!("notes-folder-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&scratch).unwrap_or_else(|e| panic!("{e}"));
        let repository = SqliteNoteRepository::open(
            scratch.join("db.sqlite3"),
            DeviceId::new("device"),
            Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([7; 32]))),
        )
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|e| panic!("{e}"));
        let files = ProjectFiles::new(
            scratch.join("attachments"),
            DeviceId::new("device"),
            repository.sealed_files(),
        );
        Self {
            root: scratch.join("NoteTaker"),
            scratch,
            repository,
            files,
            state: FolderState::default(),
        }
    }

    /// Runs one round at a fixed time, with Anna's name known.
    fn round(&mut self) -> super::notes_folder::RoundReport {
        run_round(
            &self.repository,
            &self.files,
            &HashMap::new(),
            &self.root,
            &mut self.state,
            Utc.with_ymd_and_hms(2026, 11, 9, 12, 0, 0)
                .single()
                .unwrap_or_default(),
        )
        .unwrap_or_else(|e| panic!("{e}"))
    }

    fn meeting(&self, id: &str, title: &str) -> MeetingId {
        let meeting_id = MeetingId::new(id);
        MeetingCrdtService::new(&self.repository)
            .create_titled_meeting(
                &meeting_id,
                &MeetingTitle::parse(title).unwrap_or_else(|e| panic!("{e}")),
            )
            .unwrap_or_else(|e| panic!("{e}"));
        self.repository
            .save_transcript_segments(
                &meeting_id,
                &[TranscriptSegment::new(
                    meeting_id.clone(),
                    61_000,
                    64_000,
                    None,
                    "We ship on Friday.",
                )],
            )
            .unwrap_or_else(|e| panic!("{e}"));
        meeting_id
    }

    fn project(&self, name: &str) -> ProjectId {
        self.repository
            .create_project(&ProjectName::parse(name).unwrap_or_else(|e| panic!("{e}")))
            .unwrap_or_else(|e| panic!("{e}"))
            .project_id()
    }

    /// The only meeting folder under `parent`.
    fn meeting_dir(&self, parent: &str) -> PathBuf {
        let parent = self.root.join(parent);
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&parent)
            .unwrap_or_else(|e| panic!("{}: {e}", parent.display()))
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        entries.sort();
        entries
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("no meeting folder in {}", parent.display()))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.scratch);
    }
}

/// Reads a file in the folder, or a stored file (sealed with the fixture's key, #99).
fn read(path: &Path) -> String {
    let sealed_files = crate::adapters::storage::sealed_files::SealedFiles::new(Arc::new(
        VoiceEncryptor::new(&EncryptionKey::from_bytes([7; 32])),
    ));
    let bytes = sealed_files
        .read_all(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    String::from_utf8(bytes.to_vec()).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Makes a file look older, so a round treats it as finished.
fn age(path: &Path) {
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    file.set_modified(old).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn names_are_safe_on_every_system() {
    assert_eq!(safe_name("Q3: budget / plan?"), "Q3- budget - plan-");
    assert_eq!(safe_name("  ..  "), "Untitled");
    assert_eq!(safe_name("CON"), "CON_");
    assert_eq!(safe_name(&"x".repeat(300)).chars().count(), 80);
}

#[test]
fn the_tree_holds_projects_meetings_notes_summaries_and_transcripts() {
    let mut fixture = Fixture::new();
    let launch = fixture.project("Launch");
    let inside = fixture.meeting("m-in", "Kick-off");
    fixture
        .repository
        .move_meeting_to_project(&inside, Some(launch))
        .unwrap_or_else(|e| panic!("{e}"));
    set_notes(&fixture.repository, &inside, "## Actions").unwrap_or_else(|e| panic!("{e}"));
    edit_summary(&fixture.repository, &inside, "Shipping Friday.", Utc::now())
        .unwrap_or_else(|e| panic!("{e}"));
    fixture.meeting("m-out", "Standup");
    fixture
        .files
        .add_note(&fixture.repository, launch, "Plan", "Ship it.")
        .unwrap_or_else(|e| panic!("{e}"));

    let report = fixture.round();

    assert!(report.written >= 6, "{report:?}");
    let kickoff = fixture.meeting_dir("Launch");
    assert!(
        kickoff
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(" Kick-off")),
        "{}",
        kickoff.display()
    );
    assert_eq!(read(&kickoff.join("notes.md")), "## Actions");
    assert_eq!(read(&kickoff.join("summary.md")), "Shipping Friday.");
    let transcript = read(&kickoff.join("transcript.md"));
    assert!(transcript.contains("# Kick-off"), "{transcript}");
    assert!(
        transcript.contains("**Unattributed** · 1:01"),
        "{transcript}"
    );
    assert!(transcript.contains("We ship on Friday."), "{transcript}");
    assert_eq!(
        read(&fixture.root.join("Launch").join("Plan.md")),
        "# Plan\n\nShip it.\n"
    );
    let standup = fixture.meeting_dir("No project");
    assert_eq!(read(&standup.join("notes.md")), "");
    assert!(!standup.join("summary.md").exists());

    // A second round with nothing changed writes nothing.
    assert_eq!(fixture.round().written, 0);
}

#[test]
fn edited_notes_and_summaries_are_read_back() {
    let mut fixture = Fixture::new();
    let meeting = fixture.meeting("m-1", "Review");
    fixture.round();
    let folder = fixture.meeting_dir("No project");

    std::fs::write(folder.join("notes.md"), "Typed in an editor.")
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(folder.join("summary.md"), "Written by hand.").unwrap_or_else(|e| panic!("{e}"));
    age(&folder.join("summary.md"));
    let report = fixture.round();

    assert_eq!(report.imported, 2, "{report:?}");
    assert_eq!(report.changed_meetings, vec![meeting.clone()]);
    assert_eq!(
        load_notes(&fixture.repository, &meeting)
            .ok()
            .flatten()
            .as_deref(),
        Some("Typed in an editor.")
    );
    let summary = fixture
        .repository
        .load_crdt_document(&meeting)
        .ok()
        .and_then(|document| read_summary(&document).ok().flatten())
        .unwrap_or_else(|| panic!("the summary was imported"));
    assert_eq!(summary.text, "Written by hand.");
    assert!(summary.edited);
    assert_eq!(fixture.round().imported, 0, "imported once");
}

#[test]
fn the_transcript_deletions_and_renames_are_restored() {
    let mut fixture = Fixture::new();
    let launch = fixture.project("Launch");
    fixture.meeting("m-1", "Review");
    fixture
        .files
        .add_note(&fixture.repository, launch, "Plan", "Ship it.")
        .unwrap_or_else(|e| panic!("{e}"));
    fixture.round();
    let folder = fixture.meeting_dir("No project");
    let transcript = folder.join("transcript.md");
    let original = read(&transcript);

    let mut permissions = std::fs::metadata(&transcript)
        .unwrap_or_else(|e| panic!("{e}"))
        .permissions();
    assert!(
        permissions.readonly(),
        "the transcript is written read-only"
    );
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&transcript, permissions).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&transcript, "garbled").unwrap_or_else(|e| panic!("{e}"));
    std::fs::remove_file(folder.join("notes.md")).unwrap_or_else(|e| panic!("{e}"));
    let plan = fixture.root.join("Launch").join("Plan.md");
    std::fs::rename(&plan, fixture.root.join("Launch").join("Renamed.md"))
        .unwrap_or_else(|e| panic!("{e}"));
    age(&fixture.root.join("Launch").join("Renamed.md"));
    let report = fixture.round();

    assert_eq!(read(&transcript), original);
    assert!(folder.join("notes.md").exists());
    assert!(plan.exists(), "the renamed note comes back");
    assert_eq!(report.imported, 0, "a rename is not a new file: {report:?}");
    assert_eq!(
        fixture
            .repository
            .project_resources(launch)
            .map(|resources| resources.len())
            .ok(),
        Some(1)
    );
}

#[test]
fn files_dropped_in_are_imported_and_edited_files_replace_their_resource() {
    let mut fixture = Fixture::new();
    let launch = fixture.project("Launch");
    fixture
        .files
        .add_note(&fixture.repository, launch, "Plan", "Ship it.")
        .unwrap_or_else(|e| panic!("{e}"));
    fixture.round();
    let project = fixture.root.join("Launch");

    std::fs::write(project.join("budget.txt"), "Budget: 10k").unwrap_or_else(|e| panic!("{e}"));
    age(&project.join("budget.txt"));
    std::fs::write(project.join(".budget.txt.swp"), "editor state")
        .unwrap_or_else(|e| panic!("{e}"));
    age(&project.join(".budget.txt.swp"));
    std::fs::write(project.join("fresh.txt"), "still being written")
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(project.join("Plan.md"), "# Plan\n\nShip it on Monday.\n")
        .unwrap_or_else(|e| panic!("{e}"));
    let report = fixture.round();

    assert_eq!(
        report.imported, 2,
        "the new file and the edited note: {report:?}"
    );
    let resources = fixture
        .repository
        .project_resources(launch)
        .unwrap_or_else(|e| panic!("{e}"));
    let mut names: Vec<String> = resources
        .iter()
        .map(|resource| resource.shown_file_name())
        .collect();
    names.sort();
    assert_eq!(names, ["Plan.md", "budget.txt"]);
    let plan = resources
        .iter()
        .find(|resource| resource.shown_file_name() == "Plan.md")
        .unwrap_or_else(|| panic!("the plan stays"));
    assert_eq!(
        read(Path::new(plan.file_path())),
        "# Plan\n\nShip it on Monday.\n"
    );
    assert_eq!(
        read(&project.join("Plan.md")),
        "# Plan\n\nShip it on Monday.\n"
    );

    // The file still being written is picked up once it settles.
    age(&project.join("fresh.txt"));
    assert_eq!(fixture.round().imported, 1);
    assert_eq!(fixture.round().imported, 0);
}

#[test]
fn renaming_a_meeting_moves_its_folder_and_a_deleted_meeting_leaves() {
    let mut fixture = Fixture::new();
    let meeting = fixture.meeting("m-1", "Review");
    fixture.round();
    let before = fixture.meeting_dir("No project");

    crate::application::meeting_rename::rename_meeting(
        &fixture.repository,
        &meeting,
        "Budget review",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    fixture.round();
    let after = fixture.meeting_dir("No project");
    assert!(!before.exists());
    assert!(
        after.to_string_lossy().ends_with(" Budget review"),
        "{}",
        after.display()
    );
    assert!(after.join("notes.md").exists());

    fixture
        .repository
        .delete_meeting(&meeting)
        .unwrap_or_else(|e| panic!("{e}"));
    fixture.round();
    assert!(
        !after.exists(),
        "files the app wrote and nobody changed go with the meeting"
    );
}

#[test]
fn settings_default_to_off() {
    let file = std::env::temp_dir().join(format!("notes-folder-settings-{}.json", Uuid::new_v4()));
    let settings = NotesFolderSettings::load(&file).unwrap_or_else(|e| panic!("{e}"));
    assert!(!settings.enabled);
    assert_eq!(settings.path, None);
    let enabled = NotesFolderSettings {
        enabled: true,
        path: Some(PathBuf::from("/home/me/NoteTaker")),
    };
    enabled.save(&file).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(NotesFolderSettings::load(&file).ok(), Some(enabled));
    let _ = std::fs::remove_file(file);
}

#[test]
fn files_dropped_in_a_meeting_become_its_attachments() {
    let mut fixture = Fixture::new();
    let meeting = fixture.meeting("m-1", "Review");
    fixture.round();
    let attachments = fixture.meeting_dir("No project").join("attachments");
    std::fs::create_dir_all(&attachments).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(attachments.join("board.txt"), "whiteboard").unwrap_or_else(|e| panic!("{e}"));
    age(&attachments.join("board.txt"));

    let report = fixture.round();

    assert_eq!(report.imported, 1, "{report:?}");
    assert_eq!(report.changed_meetings, vec![meeting.clone()]);
    let stored = fixture
        .repository
        .get_attachments_for_meeting(&meeting)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(stored.len(), 1);
    assert_eq!(read(Path::new(stored[0].file_path())), "whiteboard");
    let again = fixture.round();
    assert_eq!((again.imported, again.written), (0, 0), "{again:?}");
}

#[test]
fn a_file_found_where_the_app_writes_one_is_kept_aside() {
    let mut fixture = Fixture::new();
    let launch = fixture.project("Launch");
    fixture
        .files
        .add_note(&fixture.repository, launch, "Plan", "Ship it.")
        .unwrap_or_else(|e| panic!("{e}"));
    let project = fixture.root.join("Launch");
    std::fs::create_dir_all(&project).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(project.join("Plan.md"), "an older plan").unwrap_or_else(|e| panic!("{e}"));

    fixture.round();

    assert_eq!(read(&project.join("Plan.md")), "# Plan\n\nShip it.\n");
    assert_eq!(read(&project.join("Plan (on disk).md")), "an older plan");
}
