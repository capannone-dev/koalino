//! Checks the choice of cores and GPU against the machines we measured and made-up edge cases.

use super::compute_resources::{
    GpuBackend, GpuDevice, advertised_gpu_megabytes, choose_gpu, cores_for_gpu_submission,
    efficient_gpu_label, fast_core_count, first_gpu_on, inference_thread_count, is_efficient_gpu,
    model_fits_on_gpu,
};

const GIB: u64 = 1024 * 1024 * 1024;

/// Builds a GPU with the given position, kind and memory; all of its memory is free.
fn gpu(index: i32, is_discrete: bool, memory_bytes: u64) -> GpuDevice {
    GpuDevice {
        index,
        backend: GpuBackend::Vulkan,
        is_discrete,
        total_memory_bytes: memory_bytes,
        free_memory_bytes: memory_bytes,
    }
}

/// Builds an integrated GPU on `backend`, as a phone lists it.
fn phone_gpu(index: i32, backend: GpuBackend) -> GpuDevice {
    GpuDevice {
        backend,
        ..gpu(index, false, 0)
    }
}

#[test]
fn an_s23_uses_its_five_fast_cores() {
    let speeds = [2_016, 2_016, 2_016, 2_803, 2_803, 2_803, 2_803, 3_360];
    assert_eq!(fast_core_count(&speeds), Some(5));
    assert_eq!(inference_thread_count(8, fast_core_count(&speeds)), 5);
}

#[test]
fn equal_cores_are_all_fast() {
    assert_eq!(fast_core_count(&[3_000; 4]), Some(4));
}

#[test]
fn unknown_speeds_fall_back_to_the_usable_cores() {
    assert_eq!(fast_core_count(&[]), None);
    assert_eq!(inference_thread_count(4, None), 4);
}

#[test]
fn threads_stay_between_one_and_the_limit() {
    assert_eq!(inference_thread_count(22, Some(20)), 8);
    assert_eq!(inference_thread_count(0, None), 1);
    assert_eq!(inference_thread_count(2, Some(6)), 2);
}

#[test]
fn a_discrete_gpu_beats_an_integrated_one_listed_first() {
    let devices = [gpu(0, false, 30 * GIB), gpu(1, true, 8 * GIB)];
    assert_eq!(choose_gpu(&devices).map(|device| device.index), Some(1));
}

#[test]
fn the_largest_discrete_gpu_wins() {
    let devices = [gpu(0, true, 4 * GIB), gpu(1, true, 8 * GIB)];
    assert_eq!(choose_gpu(&devices).map(|device| device.index), Some(1));
}

#[test]
fn an_integrated_gpu_is_used_when_alone() {
    let devices = [gpu(0, false, 16 * GIB)];
    assert_eq!(choose_gpu(&devices).map(|device| device.index), Some(0));
}

#[test]
fn no_gpu_means_no_choice() {
    assert_eq!(choose_gpu(&[]), None);
}

#[test]
fn a_model_must_leave_room_on_the_gpu() {
    assert!(model_fits_on_gpu(GIB, 2 * GIB));
    assert!(!model_fits_on_gpu(GIB, GIB));
}

#[test]
fn only_a_discrete_gpu_is_advertised() {
    assert_eq!(advertised_gpu_megabytes(Some(gpu(1, true, 8 * GIB))), 8_192);
    assert_eq!(advertised_gpu_megabytes(Some(gpu(0, false, 30 * GIB))), 0);
    assert_eq!(advertised_gpu_megabytes(None), 0);
}

/// An S23 has one prime core that can go offline, so its cluster of four is the safe choice.
#[test]
fn gpu_submission_uses_the_fastest_cluster_of_at_least_two() {
    let s23 = [
        2_016_000, 2_016_000, 2_016_000, 2_803_200, 2_803_200, 2_803_200, 2_803_200, 3_360_000,
    ];
    assert_eq!(cores_for_gpu_submission(&s23), vec![3, 4, 5, 6]);
}

/// A phone with two equal big cores uses them, and one cluster alone offers nothing to choose.
#[test]
fn gpu_submission_handles_other_layouts() {
    assert_eq!(cores_for_gpu_submission(&[1, 1, 2, 2]), vec![2, 3]);
    assert_eq!(cores_for_gpu_submission(&[1, 1, 1, 1]), Vec::<usize>::new());
    assert_eq!(cores_for_gpu_submission(&[1, 2, 3]), Vec::<usize>::new());
    assert_eq!(cores_for_gpu_submission(&[]), Vec::<usize>::new());
}

/// Only Adreno generations whisper.cpp has kernels tuned for are offered. The generation is in the
/// driver's version text, as its device name is only "QUALCOMM Adreno(TM)".
#[test]
fn efficient_mode_needs_a_known_adreno() {
    assert!(is_efficient_gpu("OpenCL 3.0 Adreno(TM) 740"));
    assert!(is_efficient_gpu("OpenCL 3.0 Adreno(TM) 730"));
    assert!(is_efficient_gpu("OpenCL 3.0 Adreno(TM) 750"));
    assert!(is_efficient_gpu("OpenCL 3.0 Adreno(TM) 830"));
    assert!(!is_efficient_gpu("OpenCL 3.0 Adreno(TM) 650"));
    assert!(!is_efficient_gpu("OpenCL 3.0 Mali-G715"));
    assert!(!is_efficient_gpu("OpenCL 3.0 NEO Arc 740"));
}

/// Settings shows a short name.
#[test]
fn the_gpu_label_is_readable() {
    assert_eq!(
        efficient_gpu_label("OpenCL 3.0 Adreno(TM) 740"),
        "Adreno 740"
    );
    assert_eq!(efficient_gpu_label("Adreno 750"), "Adreno 750");
}

#[test]
fn a_phone_with_both_backends_finds_each_by_name() {
    // ggml registers Vulkan before OpenCL, so an Adreno phone lists the same GPU twice (#184).
    let devices = [
        phone_gpu(0, GpuBackend::Vulkan),
        phone_gpu(1, GpuBackend::OpenCl),
    ];
    assert_eq!(
        first_gpu_on(&devices, GpuBackend::OpenCl).map(|device| device.index),
        Some(1)
    );
    assert_eq!(
        first_gpu_on(&devices, GpuBackend::Vulkan).map(|device| device.index),
        Some(0)
    );
}

#[test]
fn a_backend_that_found_no_gpu_gives_none() {
    let mali_without_opencl = [phone_gpu(0, GpuBackend::Vulkan)];
    assert_eq!(first_gpu_on(&mali_without_opencl, GpuBackend::OpenCl), None);
    assert_eq!(first_gpu_on(&[], GpuBackend::Vulkan), None);
}

#[test]
fn backend_names_are_read_as_ggml_writes_them() {
    assert_eq!(GpuBackend::from_ggml_name("Vulkan"), GpuBackend::Vulkan);
    assert_eq!(GpuBackend::from_ggml_name("OpenCL"), GpuBackend::OpenCl);
    assert_eq!(GpuBackend::from_ggml_name("CUDA"), GpuBackend::Other);
}
