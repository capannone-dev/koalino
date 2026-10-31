//! Downloads a Whisper model from the fixed source and installs it only once its hash matches.
//!
//! Bytes go to `<file>.part`, which the loader never opens. A cancelled, interrupted or corrupted
//! download removes that file, so the previous model stays in use.

use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use sha2::{Digest, Sha256};

use crate::domain::whisper_catalog::WhisperModelEntry;

/// The fixed download location; fetching models is the only reason the app contacts this server.
pub const WHISPER_MODEL_BASE_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

/// Why a download did not install a model.
#[derive(Debug, thiserror::Error)]
pub enum ModelDownloadError {
    /// The owner cancelled it.
    #[error("the download was cancelled")]
    Cancelled,
    /// The server could not be reached or the connection dropped.
    #[error("the download server could not be reached")]
    Network,
    /// The server answered with something other than the file.
    #[error("the download server answered with status {0}")]
    Server(u16),
    /// The bytes did not match the pinned size or hash.
    #[error("the downloaded file failed verification")]
    Corrupted,
    /// The file could not be written.
    #[error("the model could not be saved: {0}")]
    Storage(io::Error),
}

/// Supplies a model's bytes, so tests need no network.
pub trait ModelByteSource {
    /// Opens the file's byte stream.
    ///
    /// Inputs: The catalog's file name.
    /// Outputs: A reader, or `Network`/`Server`.
    /// Side effects: An implementation may contact the download server.
    fn open(&self, file_name: &str) -> Result<Box<dyn Read>, ModelDownloadError>;
}

/// Reads from the fixed Hugging Face source.
pub struct HuggingFaceModelSource {
    client: reqwest::blocking::Client,
}

impl HuggingFaceModelSource {
    /// Creates the source, with a connect timeout and no limit on the transfer's length.
    ///
    /// Outputs: The source, or `Network` when no client can be built. Side effects: None.
    pub fn new() -> Result<Self, ModelDownloadError> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(20))
            .timeout(None)
            .build()
            .map_err(|_| ModelDownloadError::Network)?;
        Ok(Self { client })
    }
}

impl ModelByteSource for HuggingFaceModelSource {
    /// Requests the file. Side effects: Contacts Hugging Face; sends no user data.
    fn open(&self, file_name: &str) -> Result<Box<dyn Read>, ModelDownloadError> {
        let response = self
            .client
            .get(format!("{WHISPER_MODEL_BASE_URL}{file_name}"))
            .send()
            .map_err(|_| ModelDownloadError::Network)?;
        if !response.status().is_success() {
            return Err(ModelDownloadError::Server(response.status().as_u16()));
        }
        Ok(Box::new(response))
    }
}

/// Downloads, verifies and installs one model.
///
/// Inputs: The catalog entry, the byte source, the models folder, a cancel flag checked between
/// reads, and a callback receiving the total bytes received so far.
/// Outputs: The installed file's path, or why nothing was installed.
/// Side effects: Creates the folder, writes `<file>.part`, then renames it over the final name.
/// The partial file is removed on any failure.
pub fn download_whisper_model(
    model: &WhisperModelEntry,
    source: &dyn ModelByteSource,
    models_directory: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<PathBuf, ModelDownloadError> {
    fs::create_dir_all(models_directory).map_err(ModelDownloadError::Storage)?;
    let final_path = models_directory.join(model.file_name());
    let partial_path = models_directory.join(format!("{}.part", model.file_name()));
    let result = write_verified(model, source, &partial_path, cancel, progress)
        .and_then(|()| fs::rename(&partial_path, &final_path).map_err(ModelDownloadError::Storage));
    if result.is_err() {
        drop(fs::remove_file(&partial_path));
    }
    result.map(|()| final_path)
}

/// Streams to the partial file while hashing, then checks size and hash.
fn write_verified(
    model: &WhisperModelEntry,
    source: &dyn ModelByteSource,
    partial_path: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), ModelDownloadError> {
    let mut reader = source.open(model.file_name())?;
    let mut file = fs::File::create(partial_path).map_err(ModelDownloadError::Storage)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 256 * 1024];
    let mut received = 0_u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(ModelDownloadError::Cancelled);
        }
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(ModelDownloadError::Network),
        };
        if read == 0 {
            break;
        }
        received += read as u64;
        if received > model.size_bytes() {
            return Err(ModelDownloadError::Corrupted);
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .map_err(ModelDownloadError::Storage)?;
        progress(received);
    }
    file.sync_all().map_err(ModelDownloadError::Storage)?;
    let digest = hex_digest(&hasher.finalize());
    if received != model.size_bytes() || digest != model.sha256() {
        return Err(ModelDownloadError::Corrupted);
    }
    Ok(())
}

/// Formats bytes as lowercase hex.
fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
