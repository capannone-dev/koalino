//! Suggests a Whisper model from what a device can run, using the speeds in `DECISION_LOG.md`.
//!
//! This is advice shown in Settings: nothing changes or downloads until the owner accepts it. It
//! aims at keeping up with live recording, with margin for voice windows. Only some of these devices
//! were measured, so other devices get an estimate from their memory, GPU and core speed.

/// Memory a phone needs for `small`, below the 8 GB that a phone reports as about 7.
const PHONE_SMALL_MODEL_MEMORY_BYTES: u64 = 6 * GIB;
/// Top core speed a phone needs to run `small` without a GPU; an S23 measured 0.45 at 3.36 GHz.
const PHONE_SMALL_MODEL_CORE_KHZ: u64 = 2_800_000;
/// Memory a computer needs for `small`, below the 8 GB that a computer reports as about 7.7.
const COMPUTER_SMALL_MODEL_MEMORY_BYTES: u64 = 7 * GIB;
/// GPU memory turbo needs: 1.6 GB, half again for working buffers, and some kept free.
const TURBO_GPU_MEMORY_GIB: u64 = 4;
/// One gibibyte.
const GIB: u64 = 1024 * 1024 * 1024;

/// The GPU a device can use for transcription.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceGpu {
    /// None that transcription can use.
    None,
    /// A phone GPU that efficient mode supports.
    EfficientPhoneGpu,
    /// A graphics card with its own memory.
    Discrete {
        /// Its memory in whole gibibytes.
        memory_gib: u64,
    },
}

/// What a device offers, as far as it can be read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceProfile {
    /// True on a phone or tablet.
    pub is_phone: bool,
    /// Total memory, when known.
    pub total_memory_bytes: Option<u64>,
    /// The usable GPU.
    pub gpu: DeviceGpu,
    /// The fastest core's top speed in kHz, when known.
    pub fastest_core_khz: Option<u64>,
}

/// The model suggested for a device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WhisperSuggestion {
    /// The model's file name in the catalog.
    pub file_name: &'static str,
    /// True when the suggestion includes efficient mode.
    pub efficient: bool,
    /// Why, in plain language for Settings.
    pub reason: &'static str,
}

/// Suggests a model.
///
/// Inputs: The device's profile; unknown values count against larger models.
/// Outputs: A model from the catalog, with a reason. Side effects: None.
pub fn suggest_whisper_model(device: &DeviceProfile) -> WhisperSuggestion {
    let memory = device.total_memory_bytes.unwrap_or(0);
    if device.is_phone {
        return suggest_for_phone(device, memory);
    }
    match device.gpu {
        DeviceGpu::Discrete { memory_gib } if memory_gib >= TURBO_GPU_MEMORY_GIB => {
            WhisperSuggestion {
                file_name: "ggml-large-v3-turbo.bin",
                efficient: false,
                reason: "Your graphics card can run this very accurate model faster than you speak.",
            }
        }
        _ if memory >= COMPUTER_SMALL_MODEL_MEMORY_BYTES => WhisperSuggestion {
            file_name: "ggml-small.bin",
            efficient: false,
            reason: "Far fewer mistakes than base, and this computer should keep up with recordings.",
        },
        _ => base_model(
            "This computer has limited memory, so the built-in model is the safe choice.",
        ),
    }
}

/// Suggests a model for a phone.
///
/// Inputs: The profile and its memory in bytes, zero when unknown.
/// Outputs: The suggestion; `medium` is never suggested because it leaves no margin for live use.
/// Side effects: None.
fn suggest_for_phone(device: &DeviceProfile, memory: u64) -> WhisperSuggestion {
    if memory < PHONE_SMALL_MODEL_MEMORY_BYTES {
        return base_model(
            "This phone has limited memory, so the built-in model is the safe choice.",
        );
    }
    if device.gpu == DeviceGpu::EfficientPhoneGpu {
        return WhisperSuggestion {
            file_name: "ggml-small.bin",
            efficient: true,
            reason: "Far fewer mistakes than base. Efficient mode keeps it well ahead of live speech.",
        };
    }
    if device
        .fastest_core_khz
        .is_some_and(|speed| speed >= PHONE_SMALL_MODEL_CORE_KHZ)
    {
        return WhisperSuggestion {
            file_name: "ggml-small.bin",
            efficient: false,
            reason: "Far fewer mistakes than base, and this phone should keep up with recordings.",
        };
    }
    base_model("A larger model may fall behind live recordings on this phone.")
}

/// Suggests the model the app ships with.
///
/// Inputs: The reason. Outputs: The suggestion. Side effects: None.
fn base_model(reason: &'static str) -> WhisperSuggestion {
    WhisperSuggestion {
        file_name: "ggml-base.bin",
        efficient: false,
        reason,
    }
}
