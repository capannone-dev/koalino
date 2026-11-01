//! Checks the suggested model on the devices we measured and on made-up edge cases.

use super::whisper_catalog::find_whisper_model;
use super::whisper_suggestion::{DeviceGpu, DeviceProfile, suggest_whisper_model};

const GIB: u64 = 1024 * 1024 * 1024;

/// Builds a phone with the given memory, GPU and top core speed in kHz.
fn phone(memory_gib: u64, gpu: DeviceGpu, fastest_core_khz: Option<u64>) -> DeviceProfile {
    DeviceProfile {
        is_phone: true,
        total_memory_bytes: Some(memory_gib * GIB),
        gpu,
        fastest_core_khz,
    }
}

/// Builds a computer with the given memory and GPU.
fn computer(memory_gib: u64, gpu: DeviceGpu) -> DeviceProfile {
    DeviceProfile {
        is_phone: false,
        total_memory_bytes: Some(memory_gib * GIB),
        gpu,
        fastest_core_khz: None,
    }
}

/// An S23 measured 0.30 with `small`; `medium` reaches about 1.0 with voice windows, too tight.
#[test]
fn an_s23_gets_small_in_efficient_mode() {
    let suggestion =
        suggest_whisper_model(&phone(8, DeviceGpu::EfficientPhoneGpu, Some(3_360_000)));
    assert_eq!(suggestion.file_name, "ggml-small.bin");
    assert!(suggestion.efficient);
}

/// `small` measured 0.45 on an S23's CPU, so fast phones can run it without the GPU.
#[test]
fn a_fast_phone_without_a_known_gpu_gets_small_on_its_cpu() {
    let suggestion = suggest_whisper_model(&phone(8, DeviceGpu::None, Some(3_000_000)));
    assert_eq!(suggestion.file_name, "ggml-small.bin");
    assert!(!suggestion.efficient);
}

/// A slow, small or unknown phone stays on the bundled model.
#[test]
fn other_phones_keep_the_bundled_model() {
    for profile in [
        phone(8, DeviceGpu::None, Some(2_000_000)),
        phone(8, DeviceGpu::None, None),
        phone(4, DeviceGpu::None, Some(3_000_000)),
        phone(4, DeviceGpu::EfficientPhoneGpu, Some(3_000_000)),
    ] {
        let suggestion = suggest_whisper_model(&profile);
        assert_eq!(suggestion.file_name, "ggml-base.bin");
        assert!(!suggestion.efficient);
    }
}

/// Turbo measured 0.04 on a discrete GPU, and it needs about 2.7 GB of the GPU's memory.
#[test]
fn a_computer_with_a_large_enough_gpu_gets_turbo() {
    let suggestion = suggest_whisper_model(&computer(16, DeviceGpu::Discrete { memory_gib: 8 }));
    assert_eq!(suggestion.file_name, "ggml-large-v3-turbo.bin");
    assert!(!suggestion.efficient);
}

/// Without a capable GPU, a computer runs `small` comfortably (0.24) if it has the memory.
#[test]
fn other_computers_get_a_model_their_cpu_can_run() {
    let small_gpu = computer(16, DeviceGpu::Discrete { memory_gib: 2 });
    assert_eq!(
        suggest_whisper_model(&small_gpu).file_name,
        "ggml-small.bin"
    );
    assert_eq!(
        suggest_whisper_model(&computer(8, DeviceGpu::None)).file_name,
        "ggml-small.bin"
    );
    assert_eq!(
        suggest_whisper_model(&computer(4, DeviceGpu::None)).file_name,
        "ggml-base.bin"
    );
}

/// Unknown memory can't justify a larger model.
#[test]
fn unknown_memory_keeps_the_bundled_model() {
    let profile = DeviceProfile {
        is_phone: false,
        total_memory_bytes: None,
        gpu: DeviceGpu::None,
        fastest_core_khz: None,
    };
    assert_eq!(suggest_whisper_model(&profile).file_name, "ggml-base.bin");
}

/// Every suggestion exists in the catalog and explains itself.
#[test]
fn every_suggestion_is_in_the_catalog_and_has_a_reason() {
    for profile in [
        phone(8, DeviceGpu::EfficientPhoneGpu, Some(3_360_000)),
        phone(8, DeviceGpu::None, Some(3_000_000)),
        phone(4, DeviceGpu::None, None),
        computer(16, DeviceGpu::Discrete { memory_gib: 8 }),
        computer(8, DeviceGpu::None),
        computer(4, DeviceGpu::None),
    ] {
        let suggestion = suggest_whisper_model(&profile);
        assert!(find_whisper_model(suggestion.file_name).is_some());
        assert!(!suggestion.reason.is_empty());
    }
}
