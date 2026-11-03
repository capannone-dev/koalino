//! Times work by the CPU time of the calling thread, for the speed checks among the unit tests
//! (#81).
//!
//! Wall-clock time counts every moment the scheduler gives the thread's core to something else,
//! so on a busy machine or a shared CI runner a speed budget fails without the code being slower.
//! A thread's CPU time grows only while the thread runs, so load leaves it mostly unchanged.
//! Platforms without a per-thread CPU clock fall back to wall-clock time.

use std::time::Duration;
#[cfg(not(unix))]
use std::time::Instant;

/// A stopwatch that measures the CPU time the thread that started it has used since.
///
/// Read it on the same thread that started it: another thread's CPU clock is a different clock.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CpuStopwatch {
    /// The thread's CPU time when the stopwatch started.
    #[cfg(unix)]
    started_at: Duration,
    /// The wall-clock start, where no per-thread CPU clock exists.
    #[cfg(not(unix))]
    started_at: Instant,
}

impl CpuStopwatch {
    /// Starts a stopwatch on the calling thread.
    ///
    /// Inputs: None. Outputs: The running stopwatch.
    /// Side effects: Reads the thread's CPU clock; panics if the clock cannot be read, which only
    /// tests call.
    pub(crate) fn start() -> Self {
        Self {
            #[cfg(unix)]
            started_at: thread_cpu_time(),
            #[cfg(not(unix))]
            started_at: Instant::now(),
        }
    }

    /// Returns the CPU time this thread has used since `start`.
    ///
    /// Inputs: None; call it on the thread that started the stopwatch.
    /// Outputs: The CPU time used (wall-clock time where no per-thread CPU clock exists).
    /// Side effects: Reads the thread's CPU clock; panics if it cannot be read.
    pub(crate) fn elapsed(&self) -> Duration {
        #[cfg(unix)]
        {
            thread_cpu_time().saturating_sub(self.started_at)
        }
        #[cfg(not(unix))]
        {
            self.started_at.elapsed()
        }
    }
}

/// Reads the calling thread's CPU clock.
///
/// Inputs: None. Outputs: The CPU time the thread has used since it started.
/// Side effects: None; panics if the clock cannot be read.
#[cfg(unix)]
fn thread_cpu_time() -> Duration {
    let mut clock_reading = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `clock_reading` is a valid, writable `timespec` that outlives the call, and
    // `CLOCK_THREAD_CPUTIME_ID` exists on Linux, Android and macOS.
    let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut clock_reading) };
    assert_eq!(status, 0, "the thread's CPU clock must be readable");
    let seconds = u64::try_from(clock_reading.tv_sec).unwrap_or(0);
    let nanoseconds = u32::try_from(clock_reading.tv_nsec).unwrap_or(0);
    Duration::new(seconds, nanoseconds)
}

#[cfg(test)]
mod tests {
    use super::CpuStopwatch;
    use std::time::Duration;

    /// Busy work grows the clock; sleeping barely does, so preemption is not counted.
    #[test]
    fn the_stopwatch_counts_work_and_not_sleep() {
        let stopwatch = CpuStopwatch::start();
        std::thread::sleep(Duration::from_millis(200));
        let after_sleep = stopwatch.elapsed();
        let mut checksum = 0_u64;
        while stopwatch.elapsed() < after_sleep + Duration::from_millis(20) {
            checksum = std::hint::black_box(checksum.wrapping_add(1));
        }

        assert!(checksum > 0);
        #[cfg(unix)]
        assert!(
            after_sleep < Duration::from_millis(50),
            "a 200 ms sleep counted {after_sleep:?}"
        );
    }
}
