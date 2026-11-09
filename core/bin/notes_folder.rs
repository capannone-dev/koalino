//! The notes folder (#98): an opt-in, per-device folder of plain files mirroring projects and
//! meetings, so they can be read and edited with any editor.
//!
//! Layout under the chosen root:
//! - `<Project>/` holds the project's files and notes, and a folder per meeting in it;
//! - `No project/` holds the meetings outside projects;
//! - a meeting folder `<YYYY-MM-DD HH-MM> <Title>/` (UTC) holds `notes.md`, `summary.md` (when
//!   there is one), `transcript.md` and `attachments/`.
//!
//! Each round (`run_round`, every `ROUND_INTERVAL` and on demand) does three things:
//! 1. **Read back.** It reads back edits to `notes.md`, `summary.md` and project files. Notes and
//!    summaries go into the meeting document. An edited project file replaces its resource (the
//!    old one is deleted, the new one keeps the name), so the edit syncs like any other.
//! 2. **Import.** It imports new files: into a project from its folder, into a meeting from its
//!    `attachments/` folder, and a `summary.md` created in a meeting without one. Files still
//!    being written (changed in the last two seconds), hidden files and editor temporaries are
//!    skipped.
//! 3. **Write.** It writes the tree from the store. `transcript.md` is regenerated and
//!    read-only: its speakers and times can't survive free editing, and the app edits it per
//!    passage (#96). Deleted or renamed files come back, and a renamed copy is never imported.
//!    A meeting renamed or moved to another project moves its folder. Files the app wrote for a
//!    meeting or file that is gone are removed, unless someone changed them since.
//!
//! What was written is remembered in `FolderState` (beside the database), so edits made while
//! the app was closed are recognised at the next round. A file found where the app writes one,
//! with other content and never written by the app, is kept as `<name> (on disk)`.
//!
//! The folder is a decrypted copy of sealed data (#92), so it is off by default and the apps
//! warn when it is turned on. Nothing here logs names or text.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    application::{
        combined_meetings::MeetingListing,
        export_manager::segment_speaker_label,
        meeting_edits::{EditError, edit_summary, read_notes, set_notes},
        meeting_summary_document::read_summary,
    },
    domain::{
        errors::StorageError,
        models::{Attachment, MeetingId},
        projects::ProjectId,
    },
    project_commands::{ProjectCommandRepository, ProjectFiles, delete_resource},
};

/// How often the hosts run a round.
pub const ROUND_INTERVAL: Duration = Duration::from_secs(30);

/// A file changed more recently than this is still being written; the next round takes it.
const SETTLE: Duration = Duration::from_secs(2);

/// The folder of meetings outside projects.
const NO_PROJECT: &str = "No project";

/// The largest file imported.
const MAX_IMPORT_BYTES: u64 = 100 * 1024 * 1024;

/// The largest untracked file compared by content to spot a renamed copy.
const MAX_COMPARED_BYTES: u64 = 4 * 1024 * 1024;

/// The longest file or folder name written, in characters.
const MAX_NAME_CHARS: usize = 80;

/// Why the folder could not be kept.
#[derive(Debug, thiserror::Error)]
pub enum NotesFolderError {
    /// The folder must be named by an absolute path.
    #[error("the notes folder must be an absolute path")]
    NotAbsolute,
    /// A file or folder could not be read or written.
    #[error("the notes folder could not be written: {0}")]
    Io(String),
    /// The store could not be read or written.
    #[error("the notes could not be read or saved: {0}")]
    Storage(#[from] StorageError),
    /// The settings or state file could not be read or saved.
    #[error("the notes folder settings could not be read or saved: {0}")]
    Settings(String),
}

impl From<std::io::Error> for NotesFolderError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.kind().to_string())
    }
}

/// Whether this device keeps a notes folder, and where.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotesFolderSettings {
    /// Off by default: the folder is a decrypted copy.
    #[serde(default)]
    pub enabled: bool,
    /// The folder, absolute.
    #[serde(default)]
    pub path: Option<PathBuf>,
}

impl NotesFolderSettings {
    /// The settings file beside a database: `<db>.notes-folder.json`.
    pub fn file_for(database_path: &Path) -> PathBuf {
        sibling(database_path, "notes-folder.json")
    }

    /// Reads the settings; a missing file means off.
    ///
    /// Side effects: Reads the file.
    pub fn load(file: &Path) -> Result<Self, NotesFolderError> {
        read_json(file)
    }

    /// Saves the settings. Side effects: Replaces the file atomically.
    pub fn save(&self, file: &Path) -> Result<(), NotesFolderError> {
        write_json(file, self)
    }

    /// The folder to keep, when turned on with a path.
    pub fn active_root(&self) -> Option<&Path> {
        self.path.as_deref().filter(|_| self.enabled)
    }
}

/// Checks a folder chosen by the owner.
///
/// Outputs: The folder, or `NotAbsolute`; a filesystem root or a home folder itself is refused
/// too, since the tree would mix with everything else there. Side effects: None.
pub fn validate_root(path: &Path) -> Result<PathBuf, NotesFolderError> {
    if !path.is_absolute() || path.parent().is_none() {
        return Err(NotesFolderError::NotAbsolute);
    }
    if std::env::var_os("HOME").is_some_and(|home| Path::new(&home) == path) {
        return Err(NotesFolderError::Settings(String::from(
            "choose a folder inside your home folder, not the home folder itself",
        )));
    }
    Ok(path.to_path_buf())
}

/// Where a file in the folder comes from.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Origin {
    Notes {
        meeting_id: String,
    },
    Summary {
        meeting_id: String,
    },
    Transcript {
        meeting_id: String,
    },
    Resource {
        project_id: String,
        attachment_id: String,
    },
    MeetingAttachment {
        meeting_id: String,
        attachment_id: String,
    },
    /// A file the folder keeps away from the store: a renamed copy or a type it can't take.
    Ignored,
}

/// What the app last wrote or read at one path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct FileRecord {
    /// `sha256:<hex>` of a text file, or `meta:<size>:<modified ns>` of a copied file.
    fingerprint: String,
    /// `sha256:<hex>` of a copied file up to `MAX_COMPARED_BYTES`, to recognise it renamed or
    /// merely touched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    origin: Origin,
}

impl FileRecord {
    /// The record of a text file the app wrote or read.
    fn text(fingerprint: String, origin: Origin) -> Self {
        Self {
            fingerprint,
            content: None,
            origin,
        }
    }

    /// The record of a copied file as it is on disk now, or `None` when it can't be read.
    fn copied(path: &Path, origin: Origin) -> Option<Self> {
        let fingerprint = meta_fingerprint(path)?;
        let small = fs::metadata(path).is_ok_and(|metadata| metadata.len() <= MAX_COMPARED_BYTES);
        let content = small
            .then(|| fs::read(path).ok().map(|bytes| content_fingerprint(&bytes)))
            .flatten();
        Some(Self {
            fingerprint,
            content,
            origin,
        })
    }
}

/// What the folder remembers between rounds, beside the database.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FolderState {
    /// The folder this state describes; another folder starts afresh.
    #[serde(default)]
    root: Option<PathBuf>,
    /// Files the app wrote or imported, by path relative to the root (with `/`).
    #[serde(default)]
    files: BTreeMap<String, FileRecord>,
    /// Each meeting's folder.
    #[serde(default)]
    meeting_dirs: BTreeMap<String, String>,
    /// Each project's folder.
    #[serde(default)]
    project_dirs: BTreeMap<String, String>,
    /// Each project file's or meeting attachment's path, by attachment ID.
    #[serde(default)]
    attachment_paths: BTreeMap<String, String>,
}

impl FolderState {
    /// The state file beside a database: `<db>.notes-folder-state.json`.
    pub fn file_for(database_path: &Path) -> PathBuf {
        sibling(database_path, "notes-folder-state.json")
    }

    /// Reads the state; a missing or unreadable file starts afresh (files found are then kept as
    /// `(on disk)` copies, never lost). Side effects: Reads the file.
    pub fn load(file: &Path) -> Self {
        read_json(file).unwrap_or_default()
    }

    /// Saves the state. Side effects: Replaces the file atomically.
    pub fn save(&self, file: &Path) -> Result<(), NotesFolderError> {
        write_json(file, self)
    }
}

/// What one round did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RoundReport {
    /// Files written into the folder.
    pub written: usize,
    /// Edits and new files taken into the store.
    pub imported: usize,
    /// Meetings changed by imports, for the hosts to announce to paired devices.
    pub changed_meetings: Vec<MeetingId>,
}

impl RoundReport {
    fn changed(&mut self, meeting_id: &MeetingId) {
        self.imported += 1;
        if !self.changed_meetings.contains(meeting_id) {
            self.changed_meetings.push(meeting_id.clone());
        }
    }
}

/// Makes a name safe as a file or folder name on Linux, macOS and Windows.
///
/// Outputs: The name with path separators, reserved and control characters replaced by `-`,
/// without leading or trailing dots and spaces, at most 80 characters, never empty or a reserved
/// Windows name. Side effects: None.
pub fn safe_name(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            control if control.is_control() => '-',
            other => other,
        })
        .collect();
    let trimmed: String = replaced
        .trim_matches(|character: char| character == '.' || character.is_whitespace())
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let trimmed = trimmed.trim_end_matches(['.', ' ']).to_owned();
    if trimmed.is_empty() {
        return String::from("Untitled");
    }
    let stem = trimmed
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        let mut name = trimmed;
        let position = name.find('.').unwrap_or(name.len());
        name.insert(position, '_');
        return name;
    }
    trimmed
}

/// Picks `name`, or `name (2)`, `name (3)`... (before the extension of a file), unused among
/// `used` (compared without case, as on Windows and macOS), and marks it used.
fn unique(name: &str, used: &mut HashSet<String>, is_file: bool) -> String {
    let (stem, extension) = match (is_file, name.rfind('.')) {
        (true, Some(position)) if position > 0 => (&name[..position], &name[position..]),
        _ => (name, ""),
    };
    let mut candidate = name.to_owned();
    let mut number = 2;
    while used.contains(&candidate.to_lowercase()) {
        candidate = format!("{stem} ({number}){extension}");
        number += 1;
    }
    used.insert(candidate.to_lowercase());
    candidate
}

/// The name without a ` (n)` suffix, to recognise a folder the app numbered.
fn stem_of(name: &str) -> &str {
    match name.rfind(" (") {
        Some(position)
            if name.ends_with(')')
                && name[position + 2..name.len() - 1]
                    .chars()
                    .all(|c| c.is_ascii_digit()) =>
        {
            &name[..position]
        }
        _ => name,
    }
}

/// `sha256:<hex>` of some bytes.
fn content_fingerprint(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

/// `meta:<size>:<modified ns>` of a file, or `None` when it can't be read.
fn meta_fingerprint(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(format!("meta:{}:{modified}", metadata.len()))
}

/// Whether a file changed within `SETTLE`, so it may still be being written.
fn is_settling(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_none_or(|age| age < SETTLE)
}

/// Whether a name is a hidden file or an editor's temporary file, never imported.
fn is_temporary(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with('.')
        || lower.starts_with('~')
        || lower.ends_with('~')
        || [".swp", ".swx", ".tmp", ".part", ".crdownload", ".bak"]
            .iter()
            .any(|suffix| lower.ends_with(suffix))
        || lower == "desktop.ini"
        || lower == "thumbs.db"
}

/// A path relative to the root, with `/`.
fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

/// The parent of a relative path.
fn parent_of(relative: &str) -> &str {
    relative.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// The last part of a relative path.
fn name_of(relative: &str) -> &str {
    relative.rsplit_once('/').map_or(relative, |(_, name)| name)
}

/// The absolute path of a relative one.
fn absolute(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

/// Creates a folder (and its parents) readable only by its owner.
fn create_private_dir(path: &Path) -> Result<(), NotesFolderError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    Ok(())
}

/// Writes a file, making it writable first when the app left it read-only.
fn write_file(path: &Path, contents: &[u8], read_only: bool) -> Result<(), NotesFolderError> {
    if let Some(parent) = path.parent() {
        create_private_dir(parent)?;
    }
    set_read_only(path, false);
    fs::write(path, contents)?;
    if read_only {
        set_read_only(path, true);
    }
    Ok(())
}

/// Marks a file read-only, or writable again; a missing file is left alone.
fn set_read_only(path: &Path, read_only: bool) {
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(if read_only { 0o400 } else { 0o600 });
        }
        #[cfg(not(unix))]
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(read_only);
        let _ = fs::set_permissions(path, permissions);
    }
}

/// Moves a file found where the app writes one aside, as `<name> (on disk)`.
fn keep_aside(path: &Path) -> Result<(), NotesFolderError> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (stem, extension) = match name.rfind('.') {
        Some(position) if position > 0 => (&name[..position], &name[position..]),
        _ => (name.as_str(), ""),
    };
    let mut number = 1;
    loop {
        let suffix = if number == 1 {
            String::from(" (on disk)")
        } else {
            format!(" (on disk {number})")
        };
        let aside = path.with_file_name(format!("{stem}{suffix}{extension}"));
        if !aside.exists() {
            fs::rename(path, aside)?;
            return Ok(());
        }
        number += 1;
    }
}

/// A file the store wants in the folder.
struct Desired {
    relative: String,
    source: Source,
    origin: Origin,
}

/// Where a desired file's bytes come from.
enum Source {
    /// Text the app renders.
    Text { text: String, read_only: bool },
    /// A stored file copied as it is.
    Copy(PathBuf),
}

/// The tree the store wants.
#[derive(Default)]
struct Plan {
    files: Vec<Desired>,
    meeting_dirs: BTreeMap<String, String>,
    project_dirs: BTreeMap<String, String>,
    attachment_paths: BTreeMap<String, String>,
}

/// Runs one round: reads edits back, imports new files, then writes the tree.
///
/// Inputs: The repository, the project files (to import and replace files), saved people's
/// names (for transcripts), the folder, the state from the last round, and the current time.
/// Outputs: What was done, or why the round stopped (the state stays usable).
/// Side effects: Writes, moves and removes files under `root`; writes meetings and project files
/// in the store. The caller saves `state` and announces `changed_meetings`.
pub fn run_round<Repository>(
    repository: &Repository,
    files: &ProjectFiles,
    speaker_names: &HashMap<Uuid, String>,
    root: &Path,
    state: &mut FolderState,
    now: DateTime<Utc>,
) -> Result<RoundReport, NotesFolderError>
where
    Repository: ProjectCommandRepository + ?Sized,
{
    if !root.is_absolute() {
        return Err(NotesFolderError::NotAbsolute);
    }
    if state.root.as_deref() != Some(root) {
        *state = FolderState {
            root: Some(root.to_path_buf()),
            ..FolderState::default()
        };
    }
    create_private_dir(root)?;
    let mut report = RoundReport::default();
    read_back(repository, files, root, state, now, &mut report)?;
    import_new(repository, files, root, state, now, &mut report)?;
    let plan = plan_tree(repository, speaker_names, state)?;
    write_tree(root, state, plan, files, &mut report)?;
    if report.written > 0 || report.imported > 0 {
        tracing::info!(
            written = report.written,
            imported = report.imported,
            "notes folder updated"
        );
    }
    Ok(report)
}

/// Takes edits made in the folder into the store.
fn read_back<Repository>(
    repository: &Repository,
    files: &ProjectFiles,
    root: &Path,
    state: &mut FolderState,
    now: DateTime<Utc>,
    report: &mut RoundReport,
) -> Result<(), NotesFolderError>
where
    Repository: ProjectCommandRepository + ?Sized,
{
    let records: Vec<(String, FileRecord)> = state.files.clone().into_iter().collect();
    for (relative, record) in records {
        let path = absolute(root, &relative);
        if !path.is_file() {
            continue;
        }
        match &record.origin {
            Origin::Notes { meeting_id } | Origin::Summary { meeting_id } => {
                let Ok(bytes) = fs::read(&path) else { continue };
                let fingerprint = content_fingerprint(&bytes);
                if fingerprint == record.fingerprint {
                    continue;
                }
                let Ok(text) = String::from_utf8(bytes) else {
                    continue;
                };
                let meeting_id = MeetingId::new(meeting_id.clone());
                let saved = match record.origin {
                    Origin::Notes { .. } => set_notes(repository, &meeting_id, &text),
                    _ => edit_summary(repository, &meeting_id, &text, now).map(|_| ()),
                };
                match saved {
                    Ok(()) => {
                        state.files.insert(
                            relative,
                            FileRecord::text(fingerprint, record.origin.clone()),
                        );
                        report.changed(&meeting_id);
                    }
                    // An emptied summary or an oversized text is put back by the write.
                    Err(
                        EditError::Empty | EditError::TooLong { .. } | EditError::MeetingNotFound,
                    ) => {}
                    Err(EditError::Storage(error)) => return Err(error.into()),
                    Err(EditError::SegmentNotFound) => {}
                }
            }
            Origin::Resource {
                project_id,
                attachment_id,
            } => {
                let Some(fingerprint) = meta_fingerprint(&path) else {
                    continue;
                };
                if fingerprint == record.fingerprint {
                    continue;
                }
                if let Some(current) = FileRecord::copied(&path, record.origin.clone())
                    && current.content.is_some()
                    && current.content == record.content
                {
                    // Touched, not changed.
                    state.files.insert(relative, current);
                    continue;
                }
                let Ok(project) = Uuid::parse_str(project_id).map(ProjectId::from_uuid) else {
                    continue;
                };
                if fs::metadata(&path)
                    .map(|metadata| metadata.len())
                    .unwrap_or(u64::MAX)
                    > MAX_IMPORT_BYTES
                {
                    continue;
                }
                let contents = fs::read(&path)?;
                let imported =
                    files.add_contents(repository, project, name_of(&relative), &contents)?;
                let new_id = imported.email.attachment_id().to_string();
                // The old version goes the way a deletion in the app does, so devices follow.
                let _ = delete_resource(repository, files, project, attachment_id);
                state.attachment_paths.remove(attachment_id);
                state
                    .attachment_paths
                    .insert(new_id.clone(), relative.clone());
                let origin = Origin::Resource {
                    project_id: project_id.clone(),
                    attachment_id: new_id,
                };
                if let Some(record) = FileRecord::copied(&path, origin) {
                    state.files.insert(relative, record);
                }
                report.imported += 1;
            }
            Origin::Ignored => {
                if meta_fingerprint(&path).as_deref() != Some(record.fingerprint.as_str()) {
                    // Changed since it was set aside: look at it again.
                    state.files.remove(&relative);
                }
            }
            Origin::Transcript { .. } | Origin::MeetingAttachment { .. } => {}
        }
    }
    Ok(())
}

/// The fingerprints of every tracked file, to recognise a renamed copy.
fn known_fingerprints(state: &FolderState) -> HashSet<String> {
    state
        .files
        .values()
        .filter(|record| record.origin != Origin::Ignored)
        .flat_map(|record| [Some(record.fingerprint.clone()), record.content.clone()])
        .flatten()
        .collect()
}

/// Whether an untracked file is a copy of a tracked one (a rename), by size and time or content.
fn is_copy_of_tracked(path: &Path, known: &HashSet<String>) -> bool {
    if meta_fingerprint(path).is_some_and(|fingerprint| known.contains(&fingerprint)) {
        return true;
    }
    let small = fs::metadata(path).is_ok_and(|metadata| metadata.len() <= MAX_COMPARED_BYTES);
    small && fs::read(path).is_ok_and(|bytes| known.contains(&content_fingerprint(&bytes)))
}

/// Lists the files directly in a folder that the store doesn't know yet and may be imported.
fn new_files(root: &Path, folder: &str, state: &FolderState) -> Vec<String> {
    let Ok(entries) = fs::read_dir(absolute(root, folder)) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !is_temporary(name))
        .map(|name| join(folder, &name))
        .filter(|relative| !state.files.contains_key(relative))
        .collect();
    found.sort();
    found
}

/// Records a file the folder keeps away from the store.
fn ignore(state: &mut FolderState, root: &Path, relative: &str) {
    if let Some(record) = FileRecord::copied(&absolute(root, relative), Origin::Ignored) {
        state.files.insert(relative.to_owned(), record);
    }
}

/// Imports files dropped into project folders and meeting `attachments/` folders, and a
/// `summary.md` written in a meeting without one.
fn import_new<Repository>(
    repository: &Repository,
    files: &ProjectFiles,
    root: &Path,
    state: &mut FolderState,
    now: DateTime<Utc>,
    report: &mut RoundReport,
) -> Result<(), NotesFolderError>
where
    Repository: ProjectCommandRepository + ?Sized,
{
    let known = known_fingerprints(state);
    let projects: Vec<(String, String)> = state.project_dirs.clone().into_iter().collect();
    for (project_id, folder) in projects {
        let Ok(project) = Uuid::parse_str(&project_id).map(ProjectId::from_uuid) else {
            continue;
        };
        for relative in new_files(root, &folder, state) {
            let path = absolute(root, &relative);
            if is_settling(&path) {
                continue;
            }
            if is_copy_of_tracked(&path, &known)
                || fs::metadata(&path)
                    .map(|metadata| metadata.len())
                    .unwrap_or(u64::MAX)
                    > MAX_IMPORT_BYTES
            {
                ignore(state, root, &relative);
                continue;
            }
            let contents = fs::read(&path)?;
            match files.add_contents(repository, project, name_of(&relative), &contents) {
                Ok(imported) => {
                    let attachment_id = imported.email.attachment_id().to_string();
                    state
                        .attachment_paths
                        .insert(attachment_id.clone(), relative.clone());
                    let origin = Origin::Resource {
                        project_id: project_id.clone(),
                        attachment_id,
                    };
                    if let Some(record) = FileRecord::copied(&path, origin) {
                        state.files.insert(relative, record);
                    }
                    report.imported += 1;
                }
                Err(StorageError::Rejected { .. }) => ignore(state, root, &relative),
                Err(error) => return Err(error.into()),
            }
        }
    }
    let meetings: Vec<(String, String)> = state.meeting_dirs.clone().into_iter().collect();
    for (meeting_id, folder) in meetings {
        let meeting = MeetingId::new(meeting_id.clone());
        let summary = join(&folder, "summary.md");
        let summary_path = absolute(root, &summary);
        if !state.files.contains_key(&summary)
            && summary_path.is_file()
            && !is_settling(&summary_path)
        {
            let text = fs::read(&summary_path)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok());
            match text.map(|text| edit_summary(repository, &meeting, &text, now)) {
                Some(Ok(_)) => {
                    if let Ok(bytes) = fs::read(&summary_path) {
                        state.files.insert(
                            summary,
                            FileRecord::text(
                                content_fingerprint(&bytes),
                                Origin::Summary {
                                    meeting_id: meeting_id.clone(),
                                },
                            ),
                        );
                    }
                    report.changed(&meeting);
                }
                Some(Err(EditError::Storage(error))) => return Err(error.into()),
                _ => ignore(state, root, &summary),
            }
        }
        for relative in new_files(root, &join(&folder, "attachments"), state) {
            let path = absolute(root, &relative);
            if is_settling(&path) {
                continue;
            }
            if is_copy_of_tracked(&path, &known) {
                ignore(state, root, &relative);
                continue;
            }
            match files.attach_to_meeting(repository, &meeting, &path) {
                Ok(attachment) => {
                    let attachment_id = attachment.attachment_id().to_string();
                    state
                        .attachment_paths
                        .insert(attachment_id.clone(), relative.clone());
                    let origin = Origin::MeetingAttachment {
                        meeting_id: meeting_id.clone(),
                        attachment_id,
                    };
                    if let Some(record) = FileRecord::copied(&path, origin) {
                        state.files.insert(relative, record);
                    }
                    report.changed(&meeting);
                }
                Err(StorageError::Rejected { .. }) => ignore(state, root, &relative),
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

/// Picks a folder for an item: its folder from the last round when the name still fits and is
/// free, otherwise a new unique one.
fn assign_dir(
    previous: Option<&String>,
    parent: &str,
    wanted: &str,
    used: &mut HashSet<String>,
) -> String {
    if let Some(previous) = previous
        && parent_of(previous) == parent
        && stem_of(name_of(previous)) == wanted
        && !used.contains(&name_of(previous).to_lowercase())
    {
        used.insert(name_of(previous).to_lowercase());
        return previous.clone();
    }
    join(parent, &unique(wanted, used, false))
}

/// Picks a file's path: its path from the last round when it is still in `folder` and free,
/// otherwise a new unique one.
fn assign_file(
    previous: Option<&String>,
    folder: &str,
    wanted: &str,
    used: &mut HashSet<String>,
) -> String {
    if let Some(previous) = previous
        && parent_of(previous) == folder
        && !used.contains(&name_of(previous).to_lowercase())
    {
        used.insert(name_of(previous).to_lowercase());
        return previous.clone();
    }
    join(folder, &unique(&safe_name(wanted), used, true))
}

/// Works out the tree the store wants.
fn plan_tree<Repository>(
    repository: &Repository,
    speaker_names: &HashMap<Uuid, String>,
    state: &FolderState,
) -> Result<Plan, NotesFolderError>
where
    Repository: ProjectCommandRepository + ?Sized,
{
    let mut plan = Plan::default();
    let mut used_at_root: HashSet<String> = HashSet::from([NO_PROJECT.to_lowercase()]);
    let mut used_in: HashMap<String, HashSet<String>> = HashMap::new();
    let projects = repository.list_projects()?;
    for summary in &projects {
        let project_id = summary.project.project_id().to_string();
        let folder = assign_dir(
            state.project_dirs.get(&project_id),
            "",
            &safe_name(summary.project.name().as_str()),
            &mut used_at_root,
        );
        plan.project_dirs.insert(project_id, folder);
    }
    // Meetings first, so files never take a meeting's folder name.
    let listing = MeetingListing::of(&repository.meeting_recordings()?);
    let meeting_projects: HashMap<MeetingId, ProjectId> =
        repository.meeting_projects()?.into_iter().collect();
    let mut meetings = Vec::new();
    for meeting_id in repository.list_meeting_ids()? {
        if !listing.is_listed(&meeting_id) {
            continue;
        }
        if let Some(metadata) = repository.load_meeting_metadata(&meeting_id)? {
            meetings.push(metadata);
        }
    }
    meetings.sort_by_key(|metadata| {
        (
            metadata.created_at(),
            metadata.meeting_id().as_str().to_owned(),
        )
    });
    for metadata in &meetings {
        let meeting_id = metadata.meeting_id();
        let parent = meeting_projects
            .get(meeting_id)
            .and_then(|project_id| plan.project_dirs.get(&project_id.to_string()))
            .cloned()
            .unwrap_or_else(|| NO_PROJECT.to_owned());
        let title = metadata.title().map_or("Meeting", |title| title.as_str());
        let wanted = safe_name(&format!(
            "{} {title}",
            metadata.created_at().format("%Y-%m-%d %H-%M")
        ));
        let used = used_in.entry(parent.clone()).or_default();
        let folder = assign_dir(
            state.meeting_dirs.get(meeting_id.as_str()),
            &parent,
            &wanted,
            used,
        );
        plan.meeting_dirs
            .insert(meeting_id.as_str().to_owned(), folder.clone());

        let document = repository.load_crdt_document(meeting_id)?;
        let origin_id = meeting_id.as_str().to_owned();
        plan.files.push(Desired {
            relative: join(&folder, "notes.md"),
            source: Source::Text {
                text: read_notes(&document).ok().flatten().unwrap_or_default(),
                read_only: false,
            },
            origin: Origin::Notes {
                meeting_id: origin_id.clone(),
            },
        });
        if let Ok(Some(summary)) = read_summary(&document) {
            plan.files.push(Desired {
                relative: join(&folder, "summary.md"),
                source: Source::Text {
                    text: summary.text,
                    read_only: false,
                },
                origin: Origin::Summary {
                    meeting_id: origin_id.clone(),
                },
            });
        }
        plan.files.push(Desired {
            relative: join(&folder, "transcript.md"),
            source: Source::Text {
                text: render_transcript(
                    repository,
                    speaker_names,
                    metadata.display_title().as_str(),
                    metadata.created_at(),
                    meeting_id,
                )?,
                read_only: true,
            },
            origin: Origin::Transcript {
                meeting_id: origin_id.clone(),
            },
        });
        let attachments_folder = join(&folder, "attachments");
        let mut used_names = HashSet::new();
        for attachment in repository.get_attachments_for_meeting(meeting_id)? {
            let attachment_id = attachment.attachment_id().to_string();
            let relative = assign_file(
                state.attachment_paths.get(&attachment_id),
                &attachments_folder,
                &attachment.shown_file_name(),
                &mut used_names,
            );
            plan.attachment_paths
                .insert(attachment_id.clone(), relative.clone());
            plan.files.push(Desired {
                relative,
                source: Source::Copy(PathBuf::from(attachment.file_path())),
                origin: Origin::MeetingAttachment {
                    meeting_id: origin_id.clone(),
                    attachment_id,
                },
            });
        }
    }
    for summary in &projects {
        let project = summary.project.project_id();
        let project_id = project.to_string();
        let Some(folder) = plan.project_dirs.get(&project_id).cloned() else {
            continue;
        };
        let used = used_in.entry(folder.clone()).or_default();
        let mut resources: Vec<Attachment> = repository.project_resources(project)?;
        resources.sort_by_key(|resource| (resource.original_timestamp(), resource.attachment_id()));
        for resource in resources {
            let attachment_id = resource.attachment_id().to_string();
            let relative = assign_file(
                state.attachment_paths.get(&attachment_id),
                &folder,
                &resource.shown_file_name(),
                used,
            );
            plan.attachment_paths
                .insert(attachment_id.clone(), relative.clone());
            plan.files.push(Desired {
                relative,
                source: Source::Copy(PathBuf::from(resource.file_path())),
                origin: Origin::Resource {
                    project_id: project_id.clone(),
                    attachment_id,
                },
            });
        }
    }
    Ok(plan)
}

/// Renders a meeting's transcript as Markdown: its title and start, then each passage with its
/// speaker and time.
fn render_transcript<Repository>(
    repository: &Repository,
    speaker_names: &HashMap<Uuid, String>,
    title: &str,
    created_at: DateTime<Utc>,
    meeting_id: &MeetingId,
) -> Result<String, NotesFolderError>
where
    Repository: ProjectCommandRepository + ?Sized,
{
    let mut text = format!(
        "# {title}\n\n{}\n\nThis file is rewritten by Note Taker; correct the transcript in the app.\n",
        created_at.format("%Y-%m-%d %H:%M UTC")
    );
    let mut segments = repository.load_transcript_segments(meeting_id)?;
    segments.sort_by_key(|segment| (segment.start_time_ms(), segment.end_time_ms()));
    if segments.is_empty() {
        if let Some(transcript) = repository.load_transcript(meeting_id)? {
            text.push('\n');
            text.push_str(transcript.content().trim());
            text.push('\n');
        }
        return Ok(text);
    }
    for segment in &segments {
        let speaker = segment_speaker_label(segment, |identity_id| {
            speaker_names.get(&identity_id).map(String::as_str)
        });
        let seconds = segment.start_time_ms() / 1_000;
        let at = if seconds >= 3_600 {
            format!(
                "{}:{:02}:{:02}",
                seconds / 3_600,
                (seconds / 60) % 60,
                seconds % 60
            )
        } else {
            format!("{}:{:02}", seconds / 60, seconds % 60)
        };
        text.push_str(&format!(
            "\n**{speaker}** · {at}\n{}\n",
            segment.text().trim()
        ));
    }
    Ok(text)
}

/// Moves folders whose item was renamed or moved, rewriting the remembered paths under them.
fn move_dirs(
    root: &Path,
    state: &mut FolderState,
    previous: &BTreeMap<String, String>,
    wanted: &BTreeMap<String, String>,
) {
    for (id, folder) in wanted {
        let Some(old) = previous.get(id) else {
            continue;
        };
        if old == folder {
            continue;
        }
        let (from, to) = (absolute(root, old), absolute(root, folder));
        if !from.is_dir() || to.exists() {
            continue;
        }
        if let Some(parent) = to.parent()
            && create_private_dir(parent).is_err()
        {
            continue;
        }
        if fs::rename(&from, &to).is_err() {
            continue;
        }
        let prefix = format!("{old}/");
        let moved: Vec<String> = state
            .files
            .keys()
            .filter(|key| key.starts_with(&prefix))
            .cloned()
            .collect();
        for key in moved {
            if let Some(record) = state.files.remove(&key) {
                state
                    .files
                    .insert(format!("{folder}/{}", &key[prefix.len()..]), record);
            }
        }
        for path in state
            .attachment_paths
            .values_mut()
            .chain(state.meeting_dirs.values_mut())
        {
            if let Some(rest) = path.strip_prefix(&prefix) {
                *path = format!("{folder}/{rest}");
            }
        }
    }
}

/// Writes the planned tree, restores what was deleted or changed where the app owns the file,
/// and removes what the store no longer has.
fn write_tree(
    root: &Path,
    state: &mut FolderState,
    plan: Plan,
    files: &ProjectFiles,
    report: &mut RoundReport,
) -> Result<(), NotesFolderError> {
    let previous_projects = std::mem::take(&mut state.project_dirs);
    move_dirs(root, state, &previous_projects, &plan.project_dirs);
    let previous_meetings = std::mem::take(&mut state.meeting_dirs);
    move_dirs(root, state, &previous_meetings, &plan.meeting_dirs);

    let wanted: HashSet<&str> = plan
        .files
        .iter()
        .map(|desired| desired.relative.as_str())
        .collect();
    for desired in &plan.files {
        let path = absolute(root, &desired.relative);
        let record = state.files.get(&desired.relative);
        match &desired.source {
            Source::Text { text, read_only } => {
                let fingerprint = content_fingerprint(text.as_bytes());
                let on_disk = fs::read(&path).ok();
                let disk_fingerprint = on_disk.as_deref().map(content_fingerprint);
                if disk_fingerprint.as_deref() == Some(fingerprint.as_str()) {
                    state.files.insert(
                        desired.relative.clone(),
                        FileRecord::text(fingerprint, desired.origin.clone()),
                    );
                    continue;
                }
                if on_disk.is_some() && record.is_none() {
                    keep_aside(&path)?;
                }
                write_file(&path, text.as_bytes(), *read_only)?;
                state.files.insert(
                    desired.relative.clone(),
                    FileRecord::text(fingerprint, desired.origin.clone()),
                );
                report.written += 1;
            }
            Source::Copy(source) => {
                if !source.is_file() {
                    // Not on this device yet (still syncing).
                    continue;
                }
                let unchanged = record.is_some_and(|record| {
                    record.origin == desired.origin
                        && meta_fingerprint(&path).as_deref() == Some(record.fingerprint.as_str())
                });
                if unchanged {
                    continue;
                }
                if path.exists() {
                    let same = match (fs::read(&path), files.sealed_files().read_all(source)) {
                        (Ok(disk), Ok(stored)) => disk == *stored,
                        _ => false,
                    };
                    if same {
                        if let Some(record) = FileRecord::copied(&path, desired.origin.clone()) {
                            state.files.insert(desired.relative.clone(), record);
                        }
                        continue;
                    }
                    if record.is_none() {
                        keep_aside(&path)?;
                    }
                }
                if let Some(parent) = path.parent() {
                    create_private_dir(parent)?;
                }
                set_read_only(&path, false);
                // The folder holds clear copies of the sealed files (#99), by the owner's choice.
                if files.sealed_files().copy_clear(source, &path).is_err() {
                    // Unreadable here (cut short or still arriving); tried again next round.
                    continue;
                }
                if let Some(record) = FileRecord::copied(&path, desired.origin.clone()) {
                    state.files.insert(desired.relative.clone(), record);
                }
                report.written += 1;
            }
        }
    }

    // Files the app wrote for something now gone: removed when unchanged, otherwise left.
    let stale: Vec<(String, FileRecord)> = state
        .files
        .iter()
        .filter(|(relative, record)| {
            !wanted.contains(relative.as_str()) && record.origin != Origin::Ignored
        })
        .map(|(relative, record)| (relative.clone(), record.clone()))
        .collect();
    for (relative, record) in stale {
        let path = absolute(root, &relative);
        let unchanged = match &record.origin {
            Origin::Notes { .. } | Origin::Summary { .. } | Origin::Transcript { .. } => {
                fs::read(&path).is_ok_and(|bytes| content_fingerprint(&bytes) == record.fingerprint)
            }
            _ => meta_fingerprint(&path).as_deref() == Some(record.fingerprint.as_str()),
        };
        if unchanged {
            set_read_only(&path, false);
            let _ = fs::remove_file(&path);
        }
        state.files.remove(&relative);
    }
    state.files.retain(|relative, record| {
        record.origin != Origin::Ignored || absolute(root, relative).exists()
    });

    // Folders of items now gone, removed once empty (deepest first).
    let mut gone: Vec<String> = previous_meetings
        .iter()
        .filter(|(id, folder)| plan.meeting_dirs.get(*id) != Some(*folder))
        .flat_map(|(_, folder)| [join(folder, "attachments"), folder.clone()])
        .chain(
            previous_projects
                .iter()
                .filter(|(id, folder)| plan.project_dirs.get(*id) != Some(*folder))
                .map(|(_, folder)| folder.clone()),
        )
        .collect();
    gone.sort_by_key(|folder| std::cmp::Reverse(folder.matches('/').count()));
    for folder in gone {
        let _ = fs::remove_dir(absolute(root, &folder));
    }

    state.meeting_dirs = plan.meeting_dirs;
    state.project_dirs = plan.project_dirs;
    state.attachment_paths = plan.attachment_paths;
    Ok(())
}

/// A file beside the database: `<db>.<suffix>`.
fn sibling(database_path: &Path, suffix: &str) -> PathBuf {
    let mut name = database_path.as_os_str().to_owned();
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}

/// Reads JSON; a missing file gives the default.
fn read_json<Value: for<'de> Deserialize<'de> + Default>(
    file: &Path,
) -> Result<Value, NotesFolderError> {
    match fs::read(file) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| NotesFolderError::Settings(error.to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Value::default()),
        Err(error) => Err(NotesFolderError::Settings(error.kind().to_string())),
    }
}

/// Writes JSON through a temporary file, so a crash never leaves half a file.
fn write_json<Value: Serialize>(file: &Path, value: &Value) -> Result<(), NotesFolderError> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| NotesFolderError::Settings(error.to_string()))?;
    let temporary = sibling(file, "tmp");
    fs::write(&temporary, bytes)
        .map_err(|error| NotesFolderError::Settings(error.kind().to_string()))?;
    fs::rename(&temporary, file)
        .map_err(|error| NotesFolderError::Settings(error.kind().to_string()))
}
