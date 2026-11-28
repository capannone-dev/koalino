//! Where the NPU engine's downloaded files live and whether they are all there (#75).
//!
//! The owner downloads them on request (the Android host verifies each archive's SHA-256 and
//! moves the extracted files into place only when complete). They sit in `<data directory>/npu`:
//! `model/` holds Qualcomm's precompiled Whisper Small, and `qnn/` Qualcomm's QNN runtime
//! libraries, which ONNX Runtime and the NPU load. Nothing here is shipped in the app.
//!
//! Each Snapdragon generation needs the model compiled for its NPU and the HTP library pair for its
//! Hexagon version (#183); `NPU_BUILDS` lists the chips whose build was checked on a real phone.

use std::path::{Path, PathBuf};

/// The precompiled model's files, in `model/`.
pub const MODEL_FILES: [&str; 5] = [
    "encoder.onnx",
    "encoder.bin",
    "decoder.onnx",
    "decoder.bin",
    "metadata.json",
];

/// The QNN runtime libraries the HTP (NPU) backend needs on every chip, in `qnn/`; each chip also
/// needs the pair from `hexagon_files`.
pub const QNN_COMMON_FILES: [&str; 3] = ["libQnnHtp.so", "libQnnSystem.so", "libQnnHtpPrepare.so"];

/// Where Qualcomm AI Hub publishes the precompiled Whisper Small builds (release v0.66.0, built with
/// QAIRT 2.50, the version of the QNN runtime the app downloads).
const AI_HUB_RELEASE: &str = "https://qaihub-public-assets.s3.us-west-2.amazonaws.com/qai-hub-models/\
models/whisper_small_quantized/releases/v0.66.0/";
/// The builds' common name, before the target.
const AI_HUB_MODEL: &str = "whisper_small_quantized-precompiled_qnn_onnx-w8a16";

/// Notes whether the engine opened at the last recording, beside the downloaded folders.
const LAST_OPEN_FILE: &str = "last-open";

/// The precompiled model for one Snapdragon chip.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NpuBuild {
    /// Android's `Build.SOC_MODEL` for the chip, such as `SM8650`.
    pub soc_model: &'static str,
    /// The chip's marketing name, for documentation and logs.
    pub chip: &'static str,
    /// AI Hub's target name in the archive's file name.
    pub target: &'static str,
    /// The archive's SHA-256, in lowercase hex.
    pub sha256: &'static str,
    /// The archive's size.
    pub size_bytes: u64,
    /// The NPU's Hexagon version, which names its QNN library pair.
    pub hexagon: u8,
}

impl NpuBuild {
    /// The archive's address. Side effects: None.
    pub fn url(&self) -> String {
        format!("{AI_HUB_RELEASE}{AI_HUB_MODEL}-{}.zip", self.target)
    }

    /// The folder inside the archive that holds the model files. Side effects: None.
    pub fn entry_prefix(&self) -> String {
        format!("{AI_HUB_MODEL}-{}/", self.target)
    }
}

/// The chips the NPU engine runs on, each with its build. The SoC and Hexagon version are the ones
/// recorded inside each build's context binary (`soc_type`, `arch`). The 7 Gen 4 build (SM7750,
/// V73) exists too but waits for a phone to test it on.
pub const NPU_BUILDS: [NpuBuild; 4] = [
    NpuBuild {
        soc_model: "SM8450",
        chip: "Snapdragon 8 Gen 1",
        target: "qualcomm_snapdragon_8gen1",
        sha256: "97abe98a4954d963b1e714ca5f19251d3e607c6d9a6a96d68310d8c628df5f93",
        size_bytes: 284_581_314,
        hexagon: 69,
    },
    NpuBuild {
        soc_model: "SM8550",
        chip: "Snapdragon 8 Gen 2",
        target: "qualcomm_qcs8550_proxy",
        sha256: "5cbdd8d050e5d66e747b16b9afec09ccd4c868115edba1ab90c3b75ca1a53437",
        size_bytes: 285_198_576,
        hexagon: 73,
    },
    NpuBuild {
        soc_model: "SM8650",
        chip: "Snapdragon 8 Gen 3",
        target: "qualcomm_snapdragon_8gen3",
        sha256: "575cafbe5b366c91dee11fc36e91a24b5fd01b454f119a2dc7174bbb1746023c",
        size_bytes: 285_196_969,
        hexagon: 75,
    },
    NpuBuild {
        soc_model: "SM8750",
        chip: "Snapdragon 8 Elite",
        target: "qualcomm_snapdragon_8_elite_for_galaxy",
        sha256: "a152488f9b668ae05f118b6e69a9d551be05e112f6947af19f70f424a33cb225",
        size_bytes: 285_116_856,
        hexagon: 79,
    },
];

/// Names a Hexagon version's QNN library pair, such as `libQnnHtpV75Stub.so` and its skel.
///
/// Inputs: The Hexagon version. Outputs: The stub, then the skel. Side effects: None.
pub fn hexagon_files(hexagon: u8) -> [String; 2] {
    [
        format!("libQnnHtpV{hexagon}Stub.so"),
        format!("libQnnHtpV{hexagon}Skel.so"),
    ]
}

/// Finds the build for a chip.
///
/// Inputs: Android's `Build.SOC_MODEL`; case and surrounding spaces are ignored. Outputs: The build,
/// or `None` for a chip without one. Side effects: None.
pub fn npu_build_for(soc_model: &str) -> Option<&'static NpuBuild> {
    let soc_model = soc_model.trim();
    NPU_BUILDS
        .iter()
        .find(|build| build.soc_model.eq_ignore_ascii_case(soc_model))
}

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

    /// Remembers whether the engine opened on the NPU at the last recording (#183), so Settings
    /// can say when this phone's NPU could not run the model.
    ///
    /// Inputs: `true` when it opened. Side effects: Writes `last-open` in the folder; a failure to
    /// write only loses the note.
    pub fn record_open(&self, opened: bool) {
        let text = if opened { "loaded" } else { "failed" };
        if let Err(error) = std::fs::write(self.directory.join(LAST_OPEN_FILE), text) {
            tracing::debug!(%error, "unable to note whether the NPU opened");
        }
    }

    /// Reads whether the engine opened at the last recording.
    ///
    /// Outputs: `Some(true)` or `Some(false)`, or `None` when it never tried since the files were
    /// downloaded. Side effects: Reads a file.
    pub fn last_open(&self) -> Option<bool> {
        match std::fs::read_to_string(self.directory.join(LAST_OPEN_FILE))
            .ok()?
            .trim()
        {
            "loaded" => Some(true),
            "failed" => Some(false),
            _ => None,
        }
    }

    /// Reports whether every model file and QNN library is present.
    ///
    /// Outputs: `true` only when all are regular files, with one known chip's library pair (the
    /// download installs the pair for this phone's chip). Side effects: Checks files.
    pub fn is_installed(&self) -> bool {
        let (model_dir, qnn_dir) = (self.model_dir(), self.qnn_dir());
        MODEL_FILES
            .iter()
            .all(|file| model_dir.join(file).is_file())
            && QNN_COMMON_FILES
                .iter()
                .all(|file| qnn_dir.join(file).is_file())
            && NPU_BUILDS.iter().any(|build| {
                hexagon_files(build.hexagon)
                    .iter()
                    .all(|file| qnn_dir.join(file).is_file())
            })
    }
}

/// Reports whether a phone's system-on-chip runs the precompiled model.
///
/// Inputs: Android's `Build.SOC_MODEL`, such as `SM8550`. Outputs: `true` for a chip in
/// `NPU_BUILDS`; a model compiled for one NPU does not load on another. Side effects: None.
pub fn supported_soc(soc_model: &str) -> bool {
    npu_build_for(soc_model).is_some()
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
