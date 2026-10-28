//! Runs local screen-share sampling beside a recording, on its own thread (Issue #49).
//!
//! Grabbing and comparing frames takes milliseconds to tens of milliseconds, so it must not run
//! on the audio path. `RecordingController` starts one `ScreenShareWatcher` per recording. Every
//! sample interval the watcher calls `ScreenShareMonitor::sample` for the recorded meeting, using
//! the meeting time and capture state the controller publishes through a `RecordingClock`. While
//! paused or interrupted it captures nothing. Stopping the watcher hands the monitor back for the
//! next recording. Failures are logged without screen contents and never end the recording.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::{application::screen_share_monitor::ScreenShareMonitor, domain::models::MeetingId};

/// The meeting time and capture state a recording shares with its watcher.
#[derive(Debug, Default)]
pub struct RecordingClock {
    /// Meeting time, in milliseconds, of the audio captured so far.
    meeting_time_ms: AtomicU64,
    /// Whether audio is being captured (not paused, interrupted or stopped).
    is_capturing: AtomicBool,
}

impl RecordingClock {
    /// Publishes the recording's current meeting time and capture state.
    ///
    /// Inputs: The meeting time in milliseconds and whether audio is being captured.
    /// Outputs: None. Side effects: Updates the values the watcher reads.
    pub fn publish(&self, meeting_time_ms: u64, is_capturing: bool) {
        self.meeting_time_ms
            .store(meeting_time_ms, Ordering::SeqCst);
        self.is_capturing.store(is_capturing, Ordering::SeqCst);
    }

    /// Returns the published meeting time, or `None` while audio is not being captured.
    fn capturing_meeting_time_ms(&self) -> Option<u64> {
        self.is_capturing
            .load(Ordering::SeqCst)
            .then(|| self.meeting_time_ms.load(Ordering::SeqCst))
    }
}

/// The sampling thread of one recording.
pub struct ScreenShareWatcher {
    /// Dropping or sending on it ends the thread at its next wake-up.
    stop_sender: mpsc::Sender<()>,
    sampling_thread: JoinHandle<ScreenShareMonitor>,
}

impl ScreenShareWatcher {
    /// Starts sampling for one recording.
    ///
    /// Inputs: The monitor (reset first, so a share seen in an earlier recording starts over),
    /// the recorded meeting, the recording's clock, and the interval between samples.
    /// Outputs: The running watcher, or the I/O error of a thread that could not start (the
    /// monitor is then lost, and the recording continues without screen capture).
    /// Side effects: Spawns the `screen-share-watcher` thread.
    pub fn start(
        mut monitor: ScreenShareMonitor,
        meeting_id: MeetingId,
        recording_clock: Arc<RecordingClock>,
        sample_interval: Duration,
    ) -> std::io::Result<Self> {
        monitor.reset();
        let (stop_sender, stop_receiver) = mpsc::channel::<()>();
        let sampling_thread = std::thread::Builder::new()
            .name(String::from("screen-share-watcher"))
            .spawn(move || {
                while let Err(RecvTimeoutError::Timeout) =
                    stop_receiver.recv_timeout(sample_interval)
                {
                    let Some(meeting_time_ms) = recording_clock.capturing_meeting_time_ms() else {
                        continue;
                    };
                    if let Err(error) = monitor.sample(&meeting_id, meeting_time_ms) {
                        tracing::warn!(
                            meeting_id = %meeting_id.as_str(),
                            error = %error,
                            "Screen-share sample failed; the recording is unaffected"
                        );
                    }
                }
                monitor
            })?;
        Ok(Self {
            stop_sender,
            sampling_thread,
        })
    }

    /// Stops sampling and waits for the thread.
    ///
    /// Inputs: None.
    /// Outputs: The monitor for the next recording, or `None` if the thread panicked.
    /// Side effects: Blocks for at most one sample in progress; no sample runs afterwards.
    pub fn stop(self) -> Option<ScreenShareMonitor> {
        // A closed channel also ends the loop, so a failed send needs no handling.
        let _ = self.stop_sender.send(());
        match self.sampling_thread.join() {
            Ok(monitor) => Some(monitor),
            Err(_) => {
                tracing::warn!("Screen-share watcher panicked; screen capture is now off");
                None
            }
        }
    }
}
