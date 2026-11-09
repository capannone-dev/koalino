//! Specifies the clear copies the app shows sealed attachments from (#99): a copy holds the
//! clear contents under the attachment's name, is reused while fresh, and the folder is emptied
//! at start-up and of copies older than `COPY_LIFETIME`.

use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

use uuid::Uuid;

use crate::{
    adapters::storage::sealed_files::SealedFiles,
    application::crypto::voice_encryptor::ephemeral_cipher,
    clear_copies::{COPY_LIFETIME, ClearCopies},
};

/// A scratch folder removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("clear-copies-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap_or_else(|error| panic!("{error}"));
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A sealed stored file named like the attachment store names them.
fn stored(
    scratch: &Scratch,
    sealed_files: &SealedFiles,
    attachment_id: Uuid,
    name: &str,
) -> PathBuf {
    let path = scratch.0.join(format!("{attachment_id}-{name}"));
    sealed_files
        .write_new(&path, &mut &b"clear contents"[..])
        .unwrap_or_else(|error| panic!("{error}"));
    path
}

#[test]
fn a_copy_holds_the_clear_contents_under_the_shown_name() {
    let scratch = Scratch::new();
    let sealed_files = SealedFiles::new(ephemeral_cipher());
    let attachment_id = Uuid::new_v4();
    let stored_path = stored(&scratch, &sealed_files, attachment_id, "board photo.jpg");
    let copies = ClearCopies::new(scratch.0.join("copies"));

    let copy = copies
        .copy(&sealed_files, attachment_id, &stored_path)
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(copy.starts_with(scratch.0.join("copies")));
    assert!(
        copy.to_string_lossy().ends_with(".jpg"),
        "the extension is kept"
    );
    assert_eq!(
        std::fs::read(&copy).unwrap_or_default(),
        b"clear contents".to_vec()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(scratch.0.join("copies"))
            .map(|metadata| metadata.permissions().mode())
            .unwrap_or(0o777);
        assert_eq!(mode & 0o077, 0, "the folder is private");
    }
    // Asked again, the same copy is given.
    assert_eq!(
        copies.copy(&sealed_files, attachment_id, &stored_path).ok(),
        Some(copy)
    );
}

#[test]
fn an_unreadable_file_gives_no_copy() {
    let scratch = Scratch::new();
    let attachment_id = Uuid::new_v4();
    let stored_path = stored(
        &scratch,
        &SealedFiles::new(ephemeral_cipher()),
        attachment_id,
        "a.txt",
    );
    let copies = ClearCopies::new(scratch.0.join("copies"));

    let other_key = SealedFiles::new(ephemeral_cipher());
    assert!(
        copies
            .copy(&other_key, attachment_id, &stored_path)
            .is_err()
    );
    let left = std::fs::read_dir(scratch.0.join("copies"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(left, 0, "no partial copy is left");
}

#[test]
fn copies_are_wiped_at_start_and_when_old() {
    let scratch = Scratch::new();
    let sealed_files = SealedFiles::new(ephemeral_cipher());
    let copies = ClearCopies::new(scratch.0.join("copies"));
    let first_id = Uuid::new_v4();
    let first = copies
        .copy(
            &sealed_files,
            first_id,
            &stored(&scratch, &sealed_files, first_id, "a.txt"),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    // Made long ago.
    let long_ago = SystemTime::now() - COPY_LIFETIME - Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(&first)
        .and_then(|file| file.set_modified(long_ago))
        .unwrap_or_else(|error| panic!("{error}"));
    let second_id = Uuid::new_v4();
    let second = copies
        .copy(
            &sealed_files,
            second_id,
            &stored(&scratch, &sealed_files, second_id, "b.txt"),
        )
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(
        !first.exists(),
        "an old copy is removed when another is made"
    );
    assert!(second.exists());

    copies.wipe();
    assert!(!second.exists(), "start-up empties the folder");
}

#[test]
fn the_c_entry_point_refuses_a_missing_or_malformed_id_before_opening_the_runtime() {
    use std::ffi::{CStr, CString};

    use crate::ffi::{note_taker_clear_copy, note_taker_free_string};

    let answer_for = |argument: *const std::ffi::c_char| {
        let answer = unsafe { note_taker_clear_copy(argument) };
        assert!(!answer.is_null());
        let text = unsafe { CStr::from_ptr(answer) }
            .to_string_lossy()
            .into_owned();
        unsafe { note_taker_free_string(answer) };
        serde_json::from_str::<serde_json::Value>(&text).unwrap_or_default()
    };
    let malformed = CString::new("../../etc/passwd").unwrap_or_default();
    for answer in [answer_for(std::ptr::null()), answer_for(malformed.as_ptr())] {
        assert_eq!(answer["ok"], false);
        assert_eq!(answer["error_code"], "invalid_argument");
    }
}
