//! Checks pinning on real threads of this test process.

use std::{
    mem::{size_of, zeroed},
    sync::mpsc,
};

use super::gpu_thread_affinity::PinnedThreads;

/// Reads a thread's CPU set as core numbers; zero means the caller.
fn cores_of(thread_id: libc::pid_t) -> Vec<usize> {
    // SAFETY: All zeroes is the empty set, and the set is valid and of the size passed.
    let set = unsafe {
        let mut set: libc::cpu_set_t = zeroed();
        assert_eq!(
            libc::sched_getaffinity(thread_id, size_of::<libc::cpu_set_t>(), &mut set),
            0
        );
        set
    };
    // SAFETY: The set is initialized, and `CPU_ISSET` only reads it.
    (0..1024)
        .filter(|&core| unsafe { libc::CPU_ISSET(core, &set) })
        .collect()
}

/// The caller and a named thread move to the core, and both return afterwards.
#[test]
fn pins_the_caller_and_named_threads_then_restores_them() {
    let (ready_sender, ready) = mpsc::channel();
    let (finish, finish_receiver) = mpsc::channel::<()>();
    let worker = std::thread::Builder::new()
        .name("FakeDriverPin".to_owned())
        .spawn(move || {
            // SAFETY: `gettid` has no preconditions.
            let thread_id = unsafe { libc::gettid() };
            ready_sender
                .send(thread_id)
                .unwrap_or_else(|e| panic!("{e}"));
            finish_receiver.recv().unwrap_or_else(|e| panic!("{e}"));
        })
        .unwrap_or_else(|e| panic!("{e}"));
    let worker_id = ready.recv().unwrap_or_else(|e| panic!("{e}"));
    let caller_before = cores_of(0);
    let worker_before = cores_of(worker_id);
    let core = caller_before[0];
    {
        let _pinned = PinnedThreads::pin(&[core], "FakeDriverPin");
        assert_eq!(cores_of(0), vec![core]);
        assert_eq!(cores_of(worker_id), vec![core]);
    }
    assert_eq!(cores_of(0), caller_before);
    assert_eq!(cores_of(worker_id), worker_before);
    finish.send(()).unwrap_or_else(|e| panic!("{e}"));
    worker.join().unwrap_or_else(|_| panic!("worker panicked"));
}

/// No cores means nothing to do.
#[test]
fn an_empty_core_list_pins_nothing() {
    let before = cores_of(0);
    let _pinned = PinnedThreads::pin(&[], "FakeDriverNone");
    assert_eq!(cores_of(0), before);
}

/// A core that doesn't exist leaves the thread as it was.
#[test]
fn an_impossible_core_leaves_threads_alone() {
    let before = cores_of(0);
    {
        let _pinned = PinnedThreads::pin(&[1000], "FakeDriverOff");
        assert_eq!(cores_of(0), before);
    }
    assert_eq!(cores_of(0), before);
}
