//! Tests that only a complete, matching file is ever installed.

use std::{
    io::{self, Cursor, Read},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

use super::whisper_model_download::{ModelByteSource, ModelDownloadError, download_whisper_model};
use crate::domain::whisper_catalog::{WHISPER_MODELS, WhisperModelEntry};

/// A catalog entry whose hash no test bytes can match.
fn model() -> &'static WhisperModelEntry {
    WHISPER_MODELS
        .iter()
        .min_by_key(|model| model.size_bytes())
        .unwrap_or_else(|| panic!("the catalog is empty"))
}

/// Creates a clean folder for one test.
fn folder(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("note-taker-model-{name}-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&path));
    path
}

struct Bytes(Vec<u8>);
impl ModelByteSource for Bytes {
    fn open(&self, _: &str) -> Result<Box<dyn Read>, ModelDownloadError> {
        Ok(Box::new(Cursor::new(self.0.clone())))
    }
}

/// Sends a few bytes, then fails like a dropped connection.
struct Dropped;
impl Read for Dropped {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::ConnectionReset))
    }
}
struct DroppedSource;
impl ModelByteSource for DroppedSource {
    fn open(&self, _: &str) -> Result<Box<dyn Read>, ModelDownloadError> {
        Ok(Box::new(Cursor::new(vec![1_u8; 10]).chain(Dropped)))
    }
}

fn assert_nothing_installed(directory: &PathBuf) {
    let entries = std::fs::read_dir(directory)
        .map(Iterator::count)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(entries, 0, "no partial or final file may remain");
}

/// Verifies wrong bytes, even of the right length, are never installed.
#[test]
fn corrupted_bytes_are_rejected_and_removed() {
    let directory = folder("corrupt");
    let size = usize::try_from(model().size_bytes()).unwrap_or_else(|e| panic!("{e}"));
    let result = download_whisper_model(
        model(),
        &Bytes(vec![0; size]),
        &directory,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert!(matches!(result, Err(ModelDownloadError::Corrupted)));
    assert_nothing_installed(&directory);
}

/// Verifies a short file is rejected.
#[test]
fn a_truncated_file_is_rejected() {
    let directory = folder("short");
    let result = download_whisper_model(
        model(),
        &Bytes(vec![0; 100]),
        &directory,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert!(matches!(result, Err(ModelDownloadError::Corrupted)));
    assert_nothing_installed(&directory);
}

/// Verifies a dropped connection leaves nothing behind, and the old model is untouched.
#[test]
fn an_interrupted_download_keeps_the_previous_file() {
    let directory = folder("dropped");
    std::fs::create_dir_all(&directory).unwrap_or_else(|e| panic!("{e}"));
    let installed = directory.join(model().file_name());
    std::fs::write(&installed, b"previous").unwrap_or_else(|e| panic!("{e}"));
    let result = download_whisper_model(
        model(),
        &DroppedSource,
        &directory,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert!(matches!(result, Err(ModelDownloadError::Network)));
    assert_eq!(
        std::fs::read(&installed).unwrap_or_else(|e| panic!("{e}")),
        b"previous"
    );
    assert!(
        !directory
            .join(format!("{}.part", model().file_name()))
            .exists()
    );
}

/// Verifies cancelling stops the download and removes the partial file.
#[test]
fn a_cancelled_download_is_removed() {
    let directory = folder("cancel");
    let cancel = AtomicBool::new(false);
    cancel.store(true, Ordering::Relaxed);
    let result = download_whisper_model(
        model(),
        &Bytes(vec![0; 100]),
        &directory,
        &cancel,
        &mut |_| {},
    );
    assert!(matches!(result, Err(ModelDownloadError::Cancelled)));
    assert_nothing_installed(&directory);
}

/// Downloads the smallest real model from the fixed source and verifies its pinned hash.
#[test]
#[ignore = "contacts Hugging Face and downloads about 32 MB"]
fn the_smallest_model_downloads_and_verifies() {
    use super::whisper_model_download::HuggingFaceModelSource;
    let directory = folder("real");
    let source = HuggingFaceModelSource::new().unwrap_or_else(|e| panic!("{e}"));
    let path = download_whisper_model(
        model(),
        &source,
        &directory,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(path.is_file());
    drop(std::fs::remove_dir_all(&directory));
}
