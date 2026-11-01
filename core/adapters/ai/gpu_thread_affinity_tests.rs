//! Checks pinning on real threads of this test process.

use std::{
    mem::{size_of, zeroed},
    sync::{Arc, Barrier, mpsc},
};

use super::gpu_thread_affinity::{PinnedThreads, threads_named};

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

/// A named thread is found even while older threads of the process exit (#78).
///
/// Listing goes through threads oldest first, so only an older thread's exit can cut it short.
#[test]
fn finds_a_named_thread_while_older_threads_exit() {
    let missed = (0..300)
        .filter(|_| !found_while_older_threads_exit())
        .count();

    assert_eq!(missed, 0, "listings that missed the thread");
}

/// Starts older threads and a named one, and lists threads while the older ones exit.
///
/// Outputs: Whether the listing found the named thread. Side effects: Starts and joins threads.
fn found_while_older_threads_exit() -> bool {
    const OLDER_THREADS: usize = 48;
    let start_exiting = Arc::new(Barrier::new(OLDER_THREADS + 1));
    let older: Vec<_> = (0..OLDER_THREADS)
        .map(|_| {
            let start_exiting = Arc::clone(&start_exiting);
            std::thread::spawn(move || {
                start_exiting.wait();
            })
        })
        .collect();
    let (ready_sender, ready) = mpsc::channel();
    let (finish, finish_receiver) = mpsc::channel::<()>();
    let worker = std::thread::Builder::new()
        .name("FakeDriverExit".to_owned())
        .spawn(move || {
            // SAFETY: `gettid` has no preconditions.
            let thread_id = unsafe { libc::gettid() };
            ready_sender
                .send(thread_id)
                .unwrap_or_else(|e| panic!("{e}"));
            finish_receiver.recv().unwrap_or_else(|e| panic!("{e}"));
        })
        .unwrap_or_else(|e| panic!("{e}"));
    let worker_id: libc::pid_t = ready.recv().unwrap_or_else(|e| panic!("{e}"));

    start_exiting.wait();
    let found = threads_named("FakeDriverExit").contains(&worker_id);

    for thread in older {
        thread.join().unwrap_or_else(|_| panic!("thread panicked"));
    }
    finish.send(()).unwrap_or_else(|e| panic!("{e}"));
    worker.join().unwrap_or_else(|_| panic!("worker panicked"));
    found
}
