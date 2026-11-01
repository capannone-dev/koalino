//! Checks the crash guard, using `mem::forget` to stand in for a process that dies during GPU work.

use std::path::{Path, PathBuf};

use super::gpu_attempt_guard::{GpuAttemptGuard, clear_gpu_block, gpu_is_blocked};

/// Creates an empty folder and returns the marker's path inside it.
fn marker_in_new_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "note-taker-gpu-guard-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder created");
    folder.join("gpu-attempt")
}

/// Removes the folder holding a marker.
fn clean_up(marker: &Path) {
    if let Some(folder) = marker.parent() {
        let _ = std::fs::remove_dir_all(folder);
    }
}

/// A process dying during GPU setup leaves the next start on the CPU.
#[test]
fn an_abort_during_gpu_setup_blocks_the_next_attempt() {
    let marker = marker_in_new_folder("abort");
    let attempt = GpuAttemptGuard::begin(&marker).expect("first attempt allowed");
    std::mem::forget(attempt);
    assert!(gpu_is_blocked(&marker));
    assert!(GpuAttemptGuard::begin(&marker).is_none());
    clean_up(&marker);
}

/// A first chunk that succeeds proves the GPU works.
#[test]
fn a_finished_chunk_clears_the_attempt() {
    let marker = marker_in_new_folder("confirm");
    GpuAttemptGuard::begin(&marker)
        .expect("first attempt allowed")
        .confirm();
    assert!(!gpu_is_blocked(&marker));
    clean_up(&marker);
}

/// Stopping before the first chunk, or a clean failure, says nothing bad about the GPU.
#[test]
fn an_orderly_end_does_not_block_the_gpu() {
    let marker = marker_in_new_folder("drop");
    drop(GpuAttemptGuard::begin(&marker).expect("first attempt allowed"));
    assert!(!gpu_is_blocked(&marker));
    clean_up(&marker);
}

/// The owner can try again, for example after a system update.
#[test]
fn trying_again_lifts_the_block() {
    let marker = marker_in_new_folder("retry");
    std::mem::forget(GpuAttemptGuard::begin(&marker).expect("first attempt allowed"));
    clear_gpu_block(&marker).expect("block lifted");
    assert!(GpuAttemptGuard::begin(&marker).is_some());
    clean_up(&marker);
}

/// If the marker can't be written, a crash couldn't be detected, so the GPU isn't tried.
#[test]
fn an_unwritable_marker_keeps_the_cpu() {
    let marker = marker_in_new_folder("unwritable").join("missing-folder/gpu-attempt");
    assert!(GpuAttemptGuard::begin(&marker).is_none());
}
