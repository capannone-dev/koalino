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
//! - not failed `AUTO_SUMMARY_ATTEMPTS` times since the process started (a meeting with nothing to
//!   summarize yet is not a failure, so notes or files added later still get it summarized).
//!
//! Nothing is sent while the owner keeps automatic summaries off or has no default connection.
//!
//! Each round first answers the summaries paired devices asked for (#107), whatever the
//! automatic-summary setting, when the owner chose a connection for them
//! (`paired_summary_connection`, off by default). A request lives in the meeting's document, so
//! it waits there while the devices are apart. This computer:
//!
//! - takes a waiting request, its own from before a restart, or another computer's that has been
//!   working for over `STALE_WORK` (it likely stopped), oldest first; not a meeting being
//!   recorded here;
//! - marks it `working`, so a second computer leaves it alone;
//! - writes the summary with the asking device's template and removes the request in one write,
//!   or marks it `failed` with the error code for the asking device to show; it is not retried
//!   until that device asks again;
//! - answers at most `PAIRED_SUMMARIES_PER_HOUR` per hour, so a paired device can't use up the
//!   owner's plan; the rest wait.
//!
//! Between rounds, a quick check every `PAIRED_REQUEST_POLL` reads only the meetings whose
//! document changed since the last check (a request synced from a phone is such a change) and
//! the requests it left for later, so an answer starts within seconds of the sync without
//! reading every meeting each time.
//!
//! Logs carry meeting IDs and error codes only.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Condvar, Mutex, PoisonError},
    thread,
    time::Duration,
};

use chrono::{DateTime, Utc};

use super::{
    llm_commands::{LlmServices, summarize_for_paired_device, summarize_with},
    meeting_summaries::PreparedSummary,
};
use crate::{
    application::{combined_meetings::HeldSpeech, meeting_summary::SummaryError},
    domain::{
        llm::{MeetingSummary, SummaryRequest, SummaryRequestState},
        models::{DeviceId, MeetingId},
    },
    ports::storage::{DocumentChanges, MeetingRecording},
};

/// How far back meetings are summarized automatically.
pub const AUTO_SUMMARY_WINDOW: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// How many failed rounds a meeting gets before it is left for a manual summary.
pub const AUTO_SUMMARY_ATTEMPTS: u32 = 3;

/// How often a round looks for due meetings without being woken.
pub const AUTO_SUMMARY_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Most summaries written for paired devices in any hour (#107).
pub const PAIRED_SUMMARIES_PER_HOUR: usize = 10;

/// How often the quick check for paired devices' requests runs between rounds.
pub const PAIRED_REQUEST_POLL: Duration = Duration::from_secs(20);

/// How long another computer may hold a request before this one takes it over.
pub const STALE_WORK: Duration = Duration::from_secs(30 * 60);

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

    /// Reads a paired device's summary request (#107). Outputs: The request, `None`, or why the
    /// document can't be read. Side effects: Reads the document.
    fn summary_request(&self, meeting_id: &MeetingId) -> Result<Option<SummaryRequest>, String>;

    /// Replaces the meeting's summary request, as when it is taken or fails.
    /// Side effects: Writes the document, which syncs to paired devices.
    fn set_summary_request(
        &self,
        meeting_id: &MeetingId,
        request: &SummaryRequest,
    ) -> Result<(), String>;

    /// Lists the meetings whose document was written after `after_revision`, here or by a sync.
    /// Side effects: Reads meeting metadata only.
    fn document_changes_since(&self, after_revision: u64) -> Result<DocumentChanges, String>;

    /// Stores the summary written for a request and removes the request, in one write.
    /// Side effects: Writes the document, which syncs to paired devices.
    fn answer_request(
        &self,
        meeting_id: &MeetingId,
        summary: &MeetingSummary,
    ) -> Result<(), SummaryError>;
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
    /// When requests of paired devices were last sent, within the last hour.
    paired_sent: VecDeque<DateTime<Utc>>,
    /// The newest document revision the last check saw; `None` before the first check.
    paired_revision: Option<u64>,
    /// Meetings with an open request this computer left for later.
    paired_deferred: HashSet<MeetingId>,
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
            paired_sent: VecDeque::new(),
            paired_revision: None,
            paired_deferred: HashSet::new(),
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
        let answered = self.answer_paired_requests(host, true);
        let settings = self.services.settings();
        if !settings.auto_summarize() || settings.default_connection().is_none() {
            return answered;
        }
        let mut written = answered;
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
            } else if answer["error_code"] == "nothing_to_summarize" {
                // Not a failure: no transcript, notes or files yet. Checked again next round, a
                // database read with nothing sent, so notes added later still get a summary.
                tracing::debug!(
                    meeting_id = %meeting_id.as_str(),
                    "nothing to summarize yet"
                );
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

    /// Runs the quick check for paired devices' requests (#107): only meetings changed since the
    /// last check and requests left for later; every meeting on the first check.
    ///
    /// Inputs: The host. Outputs: How many summaries were written. Side effects: As
    /// `answer_paired_requests`.
    pub fn check_paired_requests(&mut self, host: &dyn AutoSummaryHost) -> usize {
        self.answer_paired_requests(host, false)
    }

    /// Answers the summary requests of paired devices (#107); see the module header.
    ///
    /// Inputs: The host, and whether to read every meeting rather than the changed ones.
    /// Outputs: How many summaries were written. Side effects: Reads the settings; when the owner
    /// chose a connection for paired devices, sends each due meeting to it, stores the summary
    /// and removes the request, or marks the request failed. Blocks while the model works.
    fn answer_paired_requests(&mut self, host: &dyn AutoSummaryHost, every_meeting: bool) -> usize {
        let Some(connection_id) = self
            .services
            .settings()
            .paired_summary_connection()
            .map(|connection| connection.id().to_owned())
        else {
            return 0;
        };
        let now = (self.now)();
        let hour_ago = now - chrono::TimeDelta::hours(1);
        while self
            .paired_sent
            .front()
            .is_some_and(|sent| *sent <= hour_ago)
        {
            self.paired_sent.pop_front();
        }
        let mut written = 0;
        let due = self.requests_due(host, now, every_meeting);
        let mut left = due.len();
        for (meeting_id, request) in due {
            if self.paired_sent.len() >= PAIRED_SUMMARIES_PER_HOUR {
                tracing::info!(
                    waiting = left,
                    "paired summary requests wait for the hourly limit"
                );
                self.paired_deferred.insert(meeting_id);
                continue;
            }
            left -= 1;
            let this_device = self.device_id.as_str().to_owned();
            let mut taken = request.clone();
            taken.state = SummaryRequestState::Working {
                by: this_device.clone(),
                since: now,
            };
            if let Err(error) = host.set_summary_request(&meeting_id, &taken) {
                tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error,
                    "a paired summary request could not be taken");
                continue;
            }
            let answer = summarize_for_paired_device(
                &self.services,
                &connection_id,
                request.template.as_deref(),
                |glossary, device_name| host.prepare(&meeting_id, glossary, device_name),
                |summary| host.answer_request(&meeting_id, summary),
            );
            let error_code = answer["error_code"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned();
            if error_code != "invalid_template" {
                self.paired_sent.push_back(now);
            }
            if answer["ok"] == true {
                written += 1;
                tracing::info!(meeting_id = %meeting_id.as_str(), "summary written for a paired device");
                continue;
            }
            tracing::warn!(meeting_id = %meeting_id.as_str(), error_code = %error_code,
                "a paired device's summary failed");
            taken.state = SummaryRequestState::Failed {
                by: this_device,
                error_code,
                at: now,
            };
            if let Err(error) = host.set_summary_request(&meeting_id, &taken) {
                tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error,
                    "a failed paired summary request could not be marked");
            }
        }
        written
    }

    /// Lists the requests this computer should take now, oldest first, and keeps the open ones
    /// it leaves (another computer's recent work, a meeting being recorded) for the next check.
    fn requests_due(
        &mut self,
        host: &dyn AutoSummaryHost,
        now: DateTime<Utc>,
        every_meeting: bool,
    ) -> Vec<(MeetingId, SummaryRequest)> {
        let changes = host.document_changes_since(self.paired_revision.unwrap_or(0));
        let candidates: Vec<MeetingId> = match (&changes, self.paired_revision) {
            (Ok(changes), Some(_)) if !every_meeting => changes
                .meeting_ids
                .iter()
                .cloned()
                .chain(self.paired_deferred.drain())
                .collect::<HashSet<MeetingId>>()
                .into_iter()
                .collect(),
            _ => match host.recordings() {
                Ok(recordings) => {
                    self.paired_deferred.clear();
                    recordings
                        .into_iter()
                        .map(|recording| recording.meeting_id)
                        .collect()
                }
                Err(error) => {
                    tracing::warn!(error = %error, "meetings could not be listed for paired summaries");
                    return Vec::new();
                }
            },
        };
        match changes {
            Ok(changes) => self.paired_revision = Some(changes.latest_revision),
            Err(error) => {
                tracing::warn!(error = %error, "document changes could not be listed");
            }
        }
        let stale_before = now - chrono::TimeDelta::from_std(STALE_WORK).unwrap_or_default();
        let mut due = Vec::new();
        for meeting_id in candidates {
            let Some(request) = host.summary_request(&meeting_id).ok().flatten() else {
                continue;
            };
            let takes = match &request.state {
                SummaryRequestState::Waiting => true,
                SummaryRequestState::Working { by, since } => {
                    by == self.device_id.as_str() || *since < stale_before
                }
                SummaryRequestState::Failed { .. } => continue,
            };
            if takes && !host.is_recording(&meeting_id) {
                due.push((meeting_id, request));
            } else {
                self.paired_deferred.insert(meeting_id);
            }
        }
        due.sort_by_key(|(_, request)| request.requested_at);
        due
    }

    /// Starts the summarizer's thread.
    ///
    /// Inputs: The host, and the wake-up the process signals when a recording ends or the
    /// after-meeting pass finishes. Outputs: The thread's handle, or why it could not start.
    /// Side effects: Runs a round at once, after each wake-up, and every `AUTO_SUMMARY_INTERVAL`;
    /// between rounds, checks paired devices' requests every `PAIRED_REQUEST_POLL`.
    pub fn spawn(
        mut self,
        host: Box<dyn AutoSummaryHost + Send>,
        wake: Arc<AutoSummaryWake>,
    ) -> std::io::Result<thread::JoinHandle<()>> {
        thread::Builder::new()
            .name("auto-summaries".to_owned())
            .spawn(move || {
                let mut next_round = std::time::Instant::now();
                loop {
                    if std::time::Instant::now() >= next_round {
                        self.run_round(host.as_ref());
                        next_round = std::time::Instant::now() + AUTO_SUMMARY_INTERVAL;
                    } else {
                        self.check_paired_requests(host.as_ref());
                    }
                    if wake.wait(PAIRED_REQUEST_POLL) {
                        next_round = std::time::Instant::now();
                    }
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
    ///
    /// Outputs: Whether it was woken rather than timed out.
    pub(crate) fn wait(&self, timeout: Duration) -> bool {
        let guard = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut guard, _) = self
            .signal
            .wait_timeout_while(guard, timeout, |woken| !*woken)
            .unwrap_or_else(PoisonError::into_inner);
        let woken = *guard;
        *guard = false;
        woken
    }
}
