//! Where the NPU engine's downloaded files live and whether they are all there (#75).
//!
//! The owner downloads them on request (the Android host verifies each archive's SHA-256 and
//! moves the extracted files into place only when complete). They sit in `<data directory>/npu`:
//! `model/` holds Qualcomm's precompiled Whisper Small, and `qnn/` Qualcomm's QNN runtime
//! libraries, which ONNX Runtime and the NPU load. Nothing here is shipped in the app.

use std::path::{Path, PathBuf};

/// The precompiled model's files, in `model/`.
pub const MODEL_FILES: [&str; 5] = [
    "encoder.onnx",
    "encoder.bin",
    "decoder.onnx",
    "decoder.bin",
    "metadata.json",
];

/// The QNN runtime libraries the HTP (NPU) backend needs on a Snapdragon 8 Gen 2, in `qnn/`.
pub const QNN_FILES: [&str; 5] = [
    "libQnnHtp.so",
    "libQnnSystem.so",
    "libQnnHtpPrepare.so",
    "libQnnHtpV73Stub.so",
    "libQnnHtpV73Skel.so",
];

/// The system-on-chip the precompiled model was built for (Snapdragon 8 Gen 2, Hexagon V73).
const SUPPORTED_SOC: &str = "SM8550";

/// The NPU folder of one device.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NpuInstall {
    directory: PathBuf,
}

impl NpuInstall {
    /// Describes the folder.
    ///
    /// Inputs: `<data directory>/npu`. Outputs: The description. Side effects: None.
    pub fn new(directory: &Path) -> Self {
        Self {
            directory: directory.to_path_buf(),
        }
    }

    /// Returns the folder itself. Side effects: None.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Returns the model's folder, `model/`. Side effects: None.
    pub fn model_dir(&self) -> PathBuf {
        self.directory.join("model")
    }

    /// Returns the QNN libraries' folder, `qnn/`. Side effects: None.
    pub fn qnn_dir(&self) -> PathBuf {
        self.directory.join("qnn")
    }

    /// Reports whether every model file and QNN library is present.
    ///
    /// Outputs: `true` only when all are regular files. Side effects: Checks files.
    pub fn is_installed(&self) -> bool {
        let (model_dir, qnn_dir) = (self.model_dir(), self.qnn_dir());
        MODEL_FILES
            .iter()
            .all(|file| model_dir.join(file).is_file())
            && QNN_FILES.iter().all(|file| qnn_dir.join(file).is_file())
    }
}

/// Reports whether a phone's system-on-chip runs the precompiled model.
///
/// Inputs: Android's `Build.SOC_MODEL`, such as `SM8550`. Outputs: `true` for Snapdragon 8 Gen 2;
/// the model is compiled for its NPU and does not load on another generation. Side effects: None.
pub fn supported_soc(soc_model: &str) -> bool {
    soc_model.trim().eq_ignore_ascii_case(SUPPORTED_SOC)
}

/// Reads this phone's system-on-chip model, such as `SM8550`.
///
/// Outputs: Android's `ro.soc.model` property; `None` elsewhere or when it is empty.
/// Side effects: Reads a system property.
pub fn this_soc() -> Option<String> {
    #[cfg(target_os = "android")]
    {
        let name = c"ro.soc.model";
        let mut value = [0 as std::ffi::c_char; libc::PROP_VALUE_MAX as usize];
        // SAFETY: `name` is NUL-terminated and `value` holds PROP_VALUE_MAX bytes, the most the
        // call writes, including its terminating NUL.
        let length = unsafe { libc::__system_property_get(name.as_ptr(), value.as_mut_ptr()) };
        if length <= 0 {
            return None;
        }
        // SAFETY: the call wrote a NUL-terminated string into `value`.
        let text = unsafe { std::ffi::CStr::from_ptr(value.as_ptr()) };
        Some(text.to_string_lossy().into_owned())
    }
    #[cfg(not(target_os = "android"))]
    None
}
