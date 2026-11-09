//! Summarizes finished meetings automatically with the default AI connection (#101).
//!
//! A round, every `AUTO_SUMMARY_INTERVAL` and whenever the host wakes it, writes the summary of
//! each meeting that is due:
//!
//! - recorded on this device, so paired devices don't each summarize (and pay for) the same
//!   meeting; the summary syncs to them in the meeting document;
//! - not folded into another meeting (the meeting it was folded into is summarized instead);
//! - started within `AUTO_SUMMARY_WINDOW`, so turning the setting on does not send the whole
//!   history to a server;
//! - finished, with a final transcript: not being recorded, and its speech is not waiting for
//!   the after-meeting pass (#87, #88);
//! - without a summary, so a summary the owner wrote, asked for or edited is never replaced;
//! - not failed `AUTO_SUMMARY_ATTEMPTS` times since the process started.
//!
//! Nothing is sent while the owner keeps automatic summaries off or has no default connection.
//! Logs carry meeting IDs and error codes only.

use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, PoisonError},
    thread,
    time::Duration,
};

use chrono::{DateTime, Utc};

use super::{
    llm_commands::{LlmServices, summarize_with},
    meeting_summaries::PreparedSummary,
};
use crate::{
    application::{combined_meetings::HeldSpeech, meeting_summary::SummaryError},
    domain::{
        llm::MeetingSummary,
        models::{DeviceId, MeetingId},
    },
    ports::storage::MeetingRecording,
};

/// How far back meetings are summarized automatically.
pub const AUTO_SUMMARY_WINDOW: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// How many failed rounds a meeting gets before it is left for a manual summary.
pub const AUTO_SUMMARY_ATTEMPTS: u32 = 3;

/// How often a round looks for due meetings without being woken.
pub const AUTO_SUMMARY_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// What a round reads and writes through the process that runs it (the app or the daemon).
pub trait AutoSummaryHost {
    /// Lists every meeting's recording facts. Outputs: The list, or why it can't be read.
    fn recordings(&self) -> Result<Vec<MeetingRecording>, String>;

    /// Reports whether the meeting has a summary. Side effects: Reads its document.
    fn has_summary(&self, meeting_id: &MeetingId) -> Result<bool, String>;

    /// Reports whether the meeting is being recorded now.
    fn is_recording(&self, meeting_id: &MeetingId) -> bool;

    /// Returns the meeting's speech kept for the after-meeting pass, or `None`.
    fn held_speech(&self, meeting_id: &MeetingId) -> Option<HeldSpeech>;

    /// Reads what the summary is written from (see `meeting_summaries::prepare_summary`).
    fn prepare(
        &self,
        meeting_id: &MeetingId,
        glossary: &[String],
        device_name: &str,
    ) -> Result<PreparedSummary, SummaryError>;

    /// Stores the summary in the meeting document, which syncs to paired devices.
    fn store(&self, meeting_id: &MeetingId, summary: &MeetingSummary) -> Result<(), SummaryError>;
}

/// Lists the meetings due for an automatic summary, oldest first.
///
/// Inputs: The host, this device, the current time and the failed attempts per meeting.
/// Outputs: The due meetings (see the module header). Side effects: Reads through the host; a
/// meeting whose summary can't be read is left for the next round.
pub fn meetings_due(
    host: &dyn AutoSummaryHost,
    this_device: &DeviceId,
    now: DateTime<Utc>,
    attempts: &HashMap<MeetingId, u32>,
) -> Vec<MeetingId> {
    let recordings = match host.recordings() {
        Ok(recordings) => recordings,
        Err(error) => {
            tracing::warn!(error = %error, "meetings could not be listed for automatic summaries");
            return Vec::new();
        }
    };
    let since = now - chrono::TimeDelta::from_std(AUTO_SUMMARY_WINDOW).unwrap_or_default();
    let mut due: Vec<&MeetingRecording> = recordings
        .iter()
        .filter(|recording| {
            &recording.acquisition_device_id == this_device
                && recording.combined_into.is_none()
                && recording.created_at >= since
                && attempts
                    .get(&recording.meeting_id)
                    .is_none_or(|tried| *tried < AUTO_SUMMARY_ATTEMPTS)
                && !host.is_recording(&recording.meeting_id)
                && host
                    .held_speech(&recording.meeting_id)
                    .is_none_or(|held| held.passed_alone)
                && host.has_summary(&recording.meeting_id) == Ok(false)
        })
        .collect();
    due.sort_by_key(|recording| recording.created_at);
    due.into_iter()
        .map(|recording| recording.meeting_id.clone())
        .collect()
}

/// Writes due meetings' summaries; one per process, run by `spawn` or a test.
pub struct AutoSummarizer {
    services: Arc<LlmServices>,
    device_id: DeviceId,
    attempts: HashMap<MeetingId, u32>,
    /// The current time; replaced in tests.
    now: Box<dyn Fn() -> DateTime<Utc> + Send>,
}

impl AutoSummarizer {
    /// Creates the summarizer. Side effects: None until a round runs.
    pub fn new(services: Arc<LlmServices>, device_id: DeviceId) -> Self {
        Self {
            services,
            device_id,
            attempts: HashMap::new(),
            now: Box::new(Utc::now),
        }
    }

    /// Replaces the clock, as tests do.
    #[must_use]
    pub fn with_clock(mut self, now: Box<dyn Fn() -> DateTime<Utc> + Send>) -> Self {
        self.now = now;
        self
    }

    /// Runs one round.
    ///
    /// Inputs: The host. Outputs: How many summaries were written. Side effects: Reads the
    /// settings; when automatic summaries are on and a default connection exists, sends each due
    /// meeting to that connection's server and stores the summary. Blocks while the model works.
    pub fn run_round(&mut self, host: &dyn AutoSummaryHost) -> usize {
        let settings = self.services.settings();
        if !settings.auto_summarize() || settings.default_connection().is_none() {
            return 0;
        }
        let mut written = 0;
        for meeting_id in meetings_due(host, &self.device_id, (self.now)(), &self.attempts) {
            // The owner may turn it off while a long round runs.
            if !self.services.settings().auto_summarize() {
                break;
            }
            let answer = summarize_with(
                &self.services,
                None,
                |glossary, device_name| host.prepare(&meeting_id, glossary, device_name),
                |summary| host.store(&meeting_id, summary),
            );
            if answer["ok"] == true {
                written += 1;
                self.attempts.remove(&meeting_id);
                tracing::info!(meeting_id = %meeting_id.as_str(), "meeting summarized automatically");
            } else {
                let tried = self.attempts.entry(meeting_id.clone()).or_insert(0);
                *tried += 1;
                tracing::warn!(
                    meeting_id = %meeting_id.as_str(),
                    error_code = answer["error_code"].as_str().unwrap_or("unknown"),
                    attempt = *tried,
                    "automatic summary failed"
                );
            }
        }
        written
    }

    /// Starts the summarizer's thread.
    ///
    /// Inputs: The host, and the wake-up the process signals when a recording ends or the
    /// after-meeting pass finishes. Outputs: The thread's handle, or why it could not start.
    /// Side effects: Runs a round at once, after each wake-up, and every `AUTO_SUMMARY_INTERVAL`.
    pub fn spawn(
        mut self,
        host: Box<dyn AutoSummaryHost + Send>,
        wake: Arc<AutoSummaryWake>,
    ) -> std::io::Result<thread::JoinHandle<()>> {
        thread::Builder::new()
            .name("auto-summaries".to_owned())
            .spawn(move || {
                loop {
                    self.run_round(host.as_ref());
                    wake.wait(AUTO_SUMMARY_INTERVAL);
                }
            })
    }
}

/// Wakes the summarizer early, such as when a recording ends.
#[derive(Default)]
pub struct AutoSummaryWake {
    pending: Mutex<bool>,
    signal: Condvar,
}

impl AutoSummaryWake {
    /// Asks for a round now. Side effects: Wakes the summarizer's thread.
    pub fn wake(&self) {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = true;
        self.signal.notify_all();
    }

    /// Waits for a wake-up or `timeout`, whichever is first.
    pub(crate) fn wait(&self, timeout: Duration) {
        let guard = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut guard, _) = self
            .signal
            .wait_timeout_while(guard, timeout, |woken| !*woken)
            .unwrap_or_else(PoisonError::into_inner);
        *guard = false;
    }
}
