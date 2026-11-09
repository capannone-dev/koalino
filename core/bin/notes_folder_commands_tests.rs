//! Specifies the notes folder's Settings commands: off by default, turned on with a checked
//! folder (or the suggested one), a round at once and on demand, and failures by code.

use std::path::{Path, PathBuf};

use serde_json::Value;
use uuid::Uuid;

use super::{
    notes_folder::{FolderState, NotesFolderError, RoundReport},
    notes_folder_commands::NotesFolderKeeper,
};

/// A keeper over a scratch database path, removed afterwards.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("notes-folder-commands-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap_or_else(|e| panic!("{e}"));
        Self(directory)
    }

    fn keeper(&self) -> NotesFolderKeeper {
        NotesFolderKeeper::for_database(&self.0.join("db.sqlite3"), Some(self.0.join("home")))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A runner that counts rounds and records the folder.
fn counting(
    rounds: &mut Vec<PathBuf>,
) -> impl FnMut(&Path, &mut FolderState) -> Result<RoundReport, NotesFolderError> + '_ {
    move |root, _state| {
        rounds.push(root.to_path_buf());
        Ok(RoundReport {
            written: 3,
            imported: 0,
            changed_meetings: Vec::new(),
        })
    }
}

#[test]
fn the_folder_is_off_until_turned_on_and_then_written_at_once() {
    let scratch = Scratch::new();
    let keeper = scratch.keeper();
    let mut rounds = Vec::new();

    let status = keeper.command(r#"{"command":"get"}"#, &mut counting(&mut rounds));
    assert_eq!(status["enabled"], false);
    assert_eq!(status["path"], Value::Null);
    assert!(
        status["suggested_path"]
            .as_str()
            .is_some_and(|path| path.ends_with("NoteTaker"))
    );
    let synced = keeper.command(r#"{"command":"sync_now"}"#, &mut counting(&mut rounds));
    assert_eq!(synced["ok"], true);
    assert!(rounds.is_empty(), "no round while off");

    let folder = scratch.0.join("Notes");
    let request =
        serde_json::json!({"command": "set", "enabled": true, "path": folder}).to_string();
    let status = keeper.command(&request, &mut counting(&mut rounds));

    assert_eq!(status["enabled"], true, "{status}");
    assert_eq!(status["path"], folder.to_string_lossy().as_ref());
    assert_eq!(status["written"], 3);
    assert!(status["last_round_at"].is_string());
    assert_eq!(rounds, vec![folder.clone()]);
    // The settings outlive the keeper.
    assert_eq!(scratch.keeper().settings().path, Some(folder));
}

#[test]
fn turning_it_on_without_a_folder_uses_the_suggested_one() {
    let scratch = Scratch::new();
    let mut rounds = Vec::new();
    let status = scratch.keeper().command(
        r#"{"command":"set","enabled":true}"#,
        &mut counting(&mut rounds),
    );
    assert_eq!(
        status["path"],
        scratch
            .0
            .join("home")
            .join("NoteTaker")
            .to_string_lossy()
            .as_ref()
    );
}

#[test]
fn bad_requests_and_failed_rounds_answer_with_codes() {
    let scratch = Scratch::new();
    let keeper = scratch.keeper();
    let mut rounds = Vec::new();
    for request in [
        r#"not json"#,
        r#"{"command":"set"}"#,
        r#"{"command":"drop"}"#,
    ] {
        let refused = keeper.command(request, &mut counting(&mut rounds));
        assert_eq!(refused["error_code"], "invalid_request", "{request}");
    }
    let relative = keeper.command(
        r#"{"command":"set","enabled":true,"path":"notes"}"#,
        &mut counting(&mut rounds),
    );
    assert_eq!(relative["error_code"], "invalid_folder");
    assert!(!keeper.settings().enabled, "nothing saved");

    let folder = scratch.0.join("Notes");
    let request =
        serde_json::json!({"command": "set", "enabled": true, "path": folder}).to_string();
    let failed = keeper.command(&request, &mut |_: &Path, _: &mut FolderState| {
        Err(NotesFolderError::Io(String::from("permission denied")))
    });
    assert_eq!(failed["error_code"], "folder_failed");
    let status = keeper.command(r#"{"command":"get"}"#, &mut counting(&mut rounds));
    assert_eq!(status["last_error"], "folder_failed");
}
