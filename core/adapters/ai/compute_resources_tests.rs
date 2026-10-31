//! Checks the choice of cores and GPU against the machines we measured and made-up edge cases.

use super::compute_resources::{
    GpuDevice, advertised_gpu_megabytes, choose_gpu, fast_core_count, inference_thread_count,
    model_fits_on_gpu,
};

const GIB: u64 = 1024 * 1024 * 1024;

/// Builds a GPU with the given position, kind and memory; all of its memory is free.
fn gpu(index: i32, is_discrete: bool, memory_bytes: u64) -> GpuDevice {
    GpuDevice {
        index,
        is_discrete,
        total_memory_bytes: memory_bytes,
        free_memory_bytes: memory_bytes,
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
