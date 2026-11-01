//! Keeps the threads that feed a phone GPU together on one CPU cluster while it works.
//!
//! On an S23's Adreno 740, a decoding step on `medium` took 64 ms by default and 38 ms with the
//! submitting thread and the driver's `AdrenoOsLib` thread on one cluster. Pinning only one of them
//! doesn't give the full gain (`DECISION_LOG.md`, #75). Every failure here only costs that speed.

use std::{
    collections::BTreeSet,
    mem::{size_of, zeroed},
};

/// Prefix of the Adreno driver's worker threads.
pub const ADRENO_DRIVER_THREAD_PREFIX: &str = "AdrenoOsLib";

/// How many listings of the process's threads are combined; see `threads_named`.
const THREAD_LISTINGS: usize = 4;

/// One thread's earlier CPU set, to put back.
struct SavedAffinity {
    /// The thread; zero means the calling thread.
    thread_id: libc::pid_t,
    /// Its CPU set before pinning.
    cores: libc::cpu_set_t,
}

/// Pinned threads, restored when this is dropped.
pub struct PinnedThreads {
    saved: Vec<SavedAffinity>,
}

impl PinnedThreads {
    /// Pins the calling thread and the named driver threads.
    ///
    /// Inputs: The core numbers, and the driver threads' name prefix. An empty list pins nothing.
    /// Outputs: What to restore. Threads that refuse, such as when a core is offline, stay as
    /// they were. Threads created while this is held inherit the set from the caller.
    /// Side effects: Reads `/proc/self/task` and changes thread affinity.
    pub fn pin(cores: &[usize], driver_thread_prefix: &str) -> Self {
        let mut saved = Vec::new();
        if cores.is_empty() {
            return Self { saved };
        }
        let chosen = cpu_set(cores);
        pin_thread(0, &chosen, &mut saved);
        for thread_id in threads_named(driver_thread_prefix) {
            pin_thread(thread_id, &chosen, &mut saved);
        }
        Self { saved }
    }
}

impl Drop for PinnedThreads {
    /// Puts every pinned thread back. Side effects: Changes thread affinity.
    fn drop(&mut self) {
        for thread in &self.saved {
            // SAFETY: The set is a valid, initialized `cpu_set_t` of the size passed.
            unsafe {
                libc::sched_setaffinity(
                    thread.thread_id,
                    size_of::<libc::cpu_set_t>(),
                    &thread.cores,
                );
            }
        }
    }
}

/// Builds a CPU set.
///
/// Inputs: Core numbers. Outputs: The set. Side effects: None.
fn cpu_set(cores: &[usize]) -> libc::cpu_set_t {
    // SAFETY: `cpu_set_t` is plain data, for which all zeroes is the empty set.
    let mut set: libc::cpu_set_t = unsafe { zeroed() };
    for &core in cores {
        // SAFETY: `CPU_SET` ignores core numbers beyond the set's size.
        unsafe { libc::CPU_SET(core, &mut set) };
    }
    set
}

/// Pins one thread, recording its earlier set when that works.
///
/// Inputs: The thread, or zero for the caller; the set; the list to add to.
/// Side effects: Changes the thread's affinity.
fn pin_thread(thread_id: libc::pid_t, chosen: &libc::cpu_set_t, saved: &mut Vec<SavedAffinity>) {
    // SAFETY: `cpu_set_t` is plain data, for which all zeroes is the empty set.
    let mut earlier: libc::cpu_set_t = unsafe { zeroed() };
    let size = size_of::<libc::cpu_set_t>();
    // SAFETY: Both sets are valid and of the size passed; a thread that has gone reports an error.
    let pinned = unsafe {
        libc::sched_getaffinity(thread_id, size, &mut earlier) == 0
            && libc::sched_setaffinity(thread_id, size, chosen) == 0
    };
    if pinned {
        saved.push(SavedAffinity {
            thread_id,
            cores: earlier,
        });
    }
}

/// Lists this process's threads whose name starts with a prefix.
///
/// A listing of `/proc/self/task` ends early when an older thread exits during it, without saying
/// so, which missed live threads in about 6% of listings in `gpu_thread_affinity_tests.rs` (#78).
/// Combining several listings finds what any one of them missed.
///
/// Inputs: The prefix. Outputs: Their ids; empty when `/proc` can't be read.
/// Side effects: Reads `/proc/self/task`.
pub(super) fn threads_named(prefix: &str) -> Vec<libc::pid_t> {
    let mut thread_ids = BTreeSet::new();
    for _ in 0..THREAD_LISTINGS {
        let Ok(entries) = std::fs::read_dir("/proc/self/task") else {
            return Vec::new();
        };
        thread_ids.extend(
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().to_str()?.parse::<libc::pid_t>().ok()),
        );
    }
    thread_ids
        .into_iter()
        .filter(|thread_id| {
            std::fs::read_to_string(format!("/proc/self/task/{thread_id}/comm"))
                .is_ok_and(|name| name.starts_with(prefix))
        })
        .collect()
}
