//! Reads who is speaking from call windows beside a recording, on its own thread (#113).
//!
//! `RecordingController` starts one `SpeakerNameWatcher` per recording when the owner allows
//! reading names (the call watch's `read_names` setting). Every sample interval, while audio is
//! being captured, the watcher asks the platform's `CallWindowReader` which names call windows
//! show as speaking and adds them, at the current meeting time, to a `SpeakingTimeline`. At stop
//! the timeline is attached to the meeting through `SpeakerNamesSink`, where the voice gallery
//! finds it to suggest names for unknown voices. A failed reading is logged once, without
//! labels, and never ends the recording.

use std::{
    sync::{
        Arc,
        mpsc::{self, RecvTimeoutError},
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    domain::{models::MeetingId, speaker_names::SpeakingTimeline},
    ports::call_window::{CallWindowReader, SpeakerNamesSink},
    screen_share_watcher::RecordingClock,
};

#[cfg(test)]
#[path = "speaker_name_watcher_tests.rs"]
mod tests;

/// Creates the platform's reader on the watcher's thread (UI Automation needs COM there).
pub type CallWindowReaderFactory = Arc<dyn Fn() -> Option<Box<dyn CallWindowReader>> + Send + Sync>;

/// Name reading configured for every recording.
pub struct SpeakerNamesSetup {
    /// Makes the reader.
    pub reader_factory: CallWindowReaderFactory,
    /// Whether the owner allows reading names; asked at each recording's start.
    pub enabled: Box<dyn Fn() -> bool + Send>,
    /// Stores each recording's readings with its meeting.
    pub sink: Box<dyn SpeakerNamesSink>,
    /// Time between readings (2 s in production).
    pub sample_interval: Duration,
}

/// The reading thread of one recording.
pub struct SpeakerNameWatcher {
    /// Sending, or dropping it, ends the thread at its next wake-up.
    stop_sender: mpsc::Sender<()>,
    reading_thread: JoinHandle<SpeakingTimeline>,
}

impl SpeakerNameWatcher {
    /// Starts reading for one recording, when the owner allows it.
    ///
    /// Inputs: The setup and the recording's clock.
    /// Outputs: The running watcher; `Ok(None)` when reading is turned off; the I/O error of a
    /// thread that could not start (the recording continues without names).
    /// Side effects: Spawns the `speaker-name-watcher` thread.
    pub fn start(
        setup: &SpeakerNamesSetup,
        recording_clock: Arc<RecordingClock>,
    ) -> std::io::Result<Option<Self>> {
        if !(setup.enabled)() {
            return Ok(None);
        }
        let reader_factory = Arc::clone(&setup.reader_factory);
        let sample_interval = setup.sample_interval;
        let (stop_sender, stop_receiver) = mpsc::channel::<()>();
        let reading_thread = std::thread::Builder::new()
            .name(String::from("speaker-name-watcher"))
            .spawn(move || {
                let mut timeline = SpeakingTimeline::default();
                let Some(mut reader) = reader_factory() else {
                    return timeline;
                };
                let mut failure_logged = false;
                while let Err(RecvTimeoutError::Timeout) =
                    stop_receiver.recv_timeout(sample_interval)
                {
                    let Some(meeting_time_ms) = recording_clock.capturing_meeting_time_ms() else {
                        continue;
                    };
                    match reader.speaking_names() {
                        Ok(names) => timeline.observe(meeting_time_ms, &names),
                        Err(error) if !failure_logged => {
                            failure_logged = true;
                            tracing::warn!(error = %error, "Call window names could not be read; the recording is unaffected");
                        }
                        Err(_) => {}
                    }
                }
                timeline
            })?;
        Ok(Some(Self {
            stop_sender,
            reading_thread,
        }))
    }

    /// Stops reading and attaches what was read to the meeting.
    ///
    /// Inputs: The setup (for its sink), the recorded meeting, and the meeting time the
    /// recording started at.
    /// Outputs: None. Side effects: Waits for a reading in progress (at most the reader's time
    /// budget); attaches the readings when any name was seen; logs counts and failures only.
    pub fn finish(self, setup: &SpeakerNamesSetup, meeting_id: &MeetingId, meeting_time_ms: u64) {
        // A closed channel also ends the loop, so a failed send needs no handling.
        let _ = self.stop_sender.send(());
        let Ok(timeline) = self.reading_thread.join() else {
            tracing::warn!("Speaker-name watcher panicked; this recording keeps no names");
            return;
        };
        if timeline.is_empty() {
            return;
        }
        let span_count = timeline.spans().len();
        match setup
            .sink
            .attach_speaker_names(meeting_id, &timeline.to_vtt(), meeting_time_ms)
        {
            Ok(()) => tracing::info!(
                meeting_id = %meeting_id.as_str(),
                span_count,
                "Call window names attached to the meeting"
            ),
            Err(error) => tracing::warn!(
                meeting_id = %meeting_id.as_str(),
                error = %error,
                "Call window names could not be stored"
            ),
        }
    }
}
