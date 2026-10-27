//! Runs CLI recordings in the foreground and coordinates `record stop` across processes.
//!
//! `record start` reads PCM from a WAV file or stdin (`PcmInput`), feeds it to a
//! `RecordingController`, and finishes when the input ends, on Ctrl+C, or when another process
//! runs `record stop`. Processes coordinate through two files next to the database:
//! `<db>.recording-active` (held by the recording process; contains its PID and meeting ID)
//! and `<db>.recording-stop` (created by `record stop`). `record stop` returns once the
//! recording process has transcribed its remaining audio and released the active file. A lock
//! left by a crashed process is detected (on Linux, through `/proc`) and replaced.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use uuid::Uuid;

use crate::{
    adapters::audio::pcm_input::PcmInput, application::recording_manager::RecordingSummary,
    cli::RecordingControl, domain::models::MeetingId, recording_controller::RecordingController,
};

#[cfg(test)]
#[path = "cli_recording_tests.rs"]
mod tests;

/// How long `record stop` waits for the recording process to finish transcribing.
const DEFAULT_STOP_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// How often `record stop` checks whether the recording has finished.
const DEFAULT_STOP_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Builds the recording controller when a recording starts.
pub type ControllerBuilder = Box<dyn Fn() -> anyhow::Result<RecordingController>>;

/// Describes the recording that holds the active file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveRecordingInfo {
    process_id: u32,
    meeting_id: String,
}

impl ActiveRecordingInfo {
    /// Returns the PID of the recording process.
    pub fn process_id(&self) -> u32 {
        self.process_id
    }

    /// Returns the meeting being recorded.
    pub fn meeting_id(&self) -> &str {
        &self.meeting_id
    }
}

/// The control files of one database.
#[derive(Clone, Debug)]
pub struct RecordingControlFiles {
    active_path: PathBuf,
    stop_path: PathBuf,
}

impl RecordingControlFiles {
    /// Derives the control file paths from the database path.
    ///
    /// Inputs: The SQLite database path.
    /// Outputs: `<db>.recording-active` and `<db>.recording-stop`.
    /// Side effects: None.
    pub fn for_database(database_path: &Path) -> Self {
        let with_suffix = |suffix: &str| {
            let mut path = database_path.as_os_str().to_owned();
            path.push(suffix);
            PathBuf::from(path)
        };
        Self {
            active_path: with_suffix(".recording-active"),
            stop_path: with_suffix(".recording-stop"),
        }
    }

    /// Returns the active-recording file path.
    pub fn active_path(&self) -> &Path {
        &self.active_path
    }

    /// Claims the database for a recording.
    ///
    /// Inputs: The meeting being recorded (written into the file for `record status`).
    /// Outputs: A lock released on drop, or `AlreadyExists` naming the running recording.
    /// Side effects: Creates the active file (replacing a stale one) and deletes any leftover
    /// stop request so it cannot end the new recording.
    pub fn acquire(&self, meeting_id: &MeetingId) -> io::Result<ActiveRecordingLock> {
        for _attempt in 0..2 {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&self.active_path)
            {
                Ok(mut active_file) => {
                    write_lock_contents(&mut active_file, meeting_id)?;
                    remove_if_present(&self.stop_path)?;
                    return Ok(ActiveRecordingLock {
                        control_files: self.clone(),
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if let Some(active) = self.active_recording() {
                        return Err(io::Error::new(
                            ErrorKind::AlreadyExists,
                            format!(
                                "a recording is already running (meeting {}, pid {})",
                                active.meeting_id, active.process_id
                            ),
                        ));
                    }
                    tracing::warn!("Replacing a recording lock left by a stopped process");
                    remove_if_present(&self.active_path)?;
                }
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            ErrorKind::AlreadyExists,
            "the recording lock could not be acquired",
        ))
    }

    /// Reads the running recording, ignoring a lock whose process has exited.
    ///
    /// Inputs: None.
    /// Outputs: The recording, or `None` when no live process holds the lock.
    /// Side effects: Reads the active file.
    pub fn active_recording(&self) -> Option<ActiveRecordingInfo> {
        let contents = fs::read_to_string(&self.active_path).ok()?;
        let mut lines = contents.lines();
        let process_id = lines.next()?.trim().parse::<u32>().ok()?;
        let meeting_id = lines.next().unwrap_or_default().trim().to_owned();
        process_is_running(process_id).then_some(ActiveRecordingInfo {
            process_id,
            meeting_id,
        })
    }

    /// Reports whether `record stop` has asked the recording to finish.
    pub fn stop_requested(&self) -> bool {
        self.stop_path.exists()
    }

    /// Asks the running recording to stop and waits until it has released the lock.
    ///
    /// Inputs: The poll interval and the maximum wait.
    /// Outputs: The stopped recording, or an error when none is running or the wait times out.
    /// Side effects: Creates the stop file; removes it again once the recording has finished.
    fn request_stop(
        &self,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<ActiveRecordingInfo, String> {
        let active = self
            .active_recording()
            .ok_or_else(|| String::from("no recording is running"))?;
        File::create(&self.stop_path)
            .map_err(|error| format!("unable to request the stop: {error}"))?;
        let waiting_since = Instant::now();
        while self.active_recording().is_some() {
            if waiting_since.elapsed() > timeout {
                return Err(String::from(
                    "the recording did not finish in time; it is still transcribing",
                ));
            }
            thread::sleep(poll_interval);
        }
        let _ = remove_if_present(&self.stop_path);
        Ok(active)
    }
}

/// Holds the active file for the duration of a recording.
pub struct ActiveRecordingLock {
    control_files: RecordingControlFiles,
}

impl Drop for ActiveRecordingLock {
    /// Releases the database: removes the active and stop files.
    fn drop(&mut self) {
        let _ = remove_if_present(&self.control_files.stop_path);
        let _ = remove_if_present(&self.control_files.active_path);
    }
}

/// Writes `<pid>\n<meeting id>\n` into the active file.
///
/// Inputs: The newly created file and the meeting.
/// Outputs: `Ok(())` or the write error.
/// Side effects: Writes and flushes the file.
fn write_lock_contents(active_file: &mut File, meeting_id: &MeetingId) -> io::Result<()> {
    writeln!(active_file, "{}", std::process::id())?;
    writeln!(active_file, "{}", meeting_id.as_str())?;
    active_file.flush()
}

/// Deletes a file, treating "not found" as success.
fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Reports whether a process exists (Linux: `/proc/<pid>`; elsewhere assumed running).
fn process_is_running(process_id: u32) -> bool {
    if cfg!(target_os = "linux") {
        Path::new("/proc").join(process_id.to_string()).exists()
    } else {
        true
    }
}

/// Implements the CLI recording commands with the real pipeline.
pub struct PipelineRecordingControl {
    control_files: RecordingControlFiles,
    controller_builder: ControllerBuilder,
    /// Set by the Ctrl+C handler (or a test) to end the recording gracefully.
    interrupt_requested: Arc<AtomicBool>,
    /// Whether `record` installs a Ctrl+C handler that sets `interrupt_requested`.
    handles_ctrl_c: bool,
    stop_poll_interval: Duration,
    stop_timeout: Duration,
}

impl PipelineRecordingControl {
    /// Creates the control for one database.
    ///
    /// Inputs: The database's control files and the controller builder (called only by `record`,
    /// so other commands never compose the pipeline).
    /// Outputs: The control, handling Ctrl+C during recordings.
    /// Side effects: None.
    pub fn new(
        control_files: RecordingControlFiles,
        controller_builder: ControllerBuilder,
    ) -> Self {
        Self {
            control_files,
            controller_builder,
            interrupt_requested: Arc::new(AtomicBool::new(false)),
            handles_ctrl_c: true,
            stop_poll_interval: DEFAULT_STOP_POLL_INTERVAL,
            stop_timeout: DEFAULT_STOP_TIMEOUT,
        }
    }

    /// Uses an externally controlled interrupt flag instead of a Ctrl+C handler.
    #[cfg(test)]
    fn with_interrupt_flag(mut self, interrupt_requested: Arc<AtomicBool>) -> Self {
        self.interrupt_requested = interrupt_requested;
        self.handles_ctrl_c = false;
        self
    }

    /// Overrides how often `record stop` polls.
    #[cfg(test)]
    fn with_stop_poll_interval(mut self, stop_poll_interval: Duration) -> Self {
        self.stop_poll_interval = stop_poll_interval;
        self
    }

    /// Feeds the input to the controller until it ends or a stop is requested.
    ///
    /// Inputs: The started controller and the opened input.
    /// Outputs: `Ok(())`, or the first input or pipeline error.
    /// Side effects: Pushes PCM; chunks are transcribed in the background.
    fn capture_until_stopped(
        &self,
        controller: &mut RecordingController,
        pcm_input: &mut PcmInput,
    ) -> Result<(), String> {
        loop {
            if self.interrupt_requested.load(Ordering::SeqCst)
                || self.control_files.stop_requested()
            {
                return Ok(());
            }
            match pcm_input.read_block() {
                Ok(Some(samples)) => {
                    controller
                        .push_pcm(samples)
                        .map_err(|error| error.to_string())?;
                }
                Ok(None) => return Ok(()),
                Err(error) => return Err(format!("unable to read the audio input: {error}")),
            }
        }
    }

    /// Spawns a thread that sets the interrupt flag on Ctrl+C.
    ///
    /// Inputs: None.
    /// Outputs: None; failure to install the handler only means Ctrl+C terminates the process.
    /// Side effects: Replaces the default SIGINT behaviour for the rest of the process.
    fn install_ctrl_c_handler(&self) {
        let interrupt_requested = Arc::clone(&self.interrupt_requested);
        let spawn_result = thread::Builder::new()
            .name("ctrl-c".to_owned())
            .spawn(move || {
                let Ok(signal_runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                if signal_runtime.block_on(tokio::signal::ctrl_c()).is_ok() {
                    eprintln!("Stopping: finishing transcription of the captured audio…");
                    interrupt_requested.store(true, Ordering::SeqCst);
                }
            });
        if spawn_result.is_err() {
            tracing::warn!("Ctrl+C handler could not be installed");
        }
    }
}

impl RecordingControl for PipelineRecordingControl {
    /// Records `input` into a meeting in the foreground.
    ///
    /// Inputs: The meeting (`None` generates `meeting-<uuid>`) and a WAV path or `-` for raw
    /// stdin PCM.
    /// Outputs: A summary once every chunk is transcribed, or a readable error.
    /// Side effects: Holds the active file, composes the pipeline, persists segments, and prints
    /// progress to stderr (never audio or transcript text).
    fn record(&mut self, meeting_id: Option<MeetingId>, input: &Path) -> Result<String, String> {
        let mut pcm_input = open_input(input)?;
        let meeting_id =
            meeting_id.unwrap_or_else(|| MeetingId::new(format!("meeting-{}", Uuid::new_v4())));
        let _lock = self
            .control_files
            .acquire(&meeting_id)
            .map_err(|error| error.to_string())?;
        let mut controller = (self.controller_builder)().map_err(|error| format!("{error:#}"))?;
        controller
            .start(Some(meeting_id.clone()))
            .map_err(|error| error.to_string())?;
        if self.handles_ctrl_c {
            self.install_ctrl_c_handler();
        }
        eprintln!(
            "Recording into meeting {}. Press Ctrl+C or run `note-taker record stop` to finish.",
            meeting_id.as_str()
        );
        let capture_result = self.capture_until_stopped(&mut controller, &mut pcm_input);
        eprintln!("Waiting for the remaining audio to be transcribed…");
        let summary = controller.stop().map_err(|error| error.to_string())?;
        capture_result?;
        Ok(format_summary(&summary))
    }

    /// Stops the recording running in another process (or terminal).
    ///
    /// Inputs: None.
    /// Outputs: A confirmation once it has finished, or an error when none is running.
    /// Side effects: Creates the stop file and blocks while the recording finishes.
    fn request_stop(&mut self) -> Result<String, String> {
        let stopped = self
            .control_files
            .request_stop(self.stop_poll_interval, self.stop_timeout)?;
        Ok(format!(
            "recording of meeting {} stopped and transcribed",
            stopped.meeting_id()
        ))
    }

    /// Describes the running recording, if any.
    fn status(&self) -> Result<String, String> {
        Ok(match self.control_files.active_recording() {
            Some(active) => format!(
                "Recording (meeting {}, pid {})",
                active.meeting_id(),
                active.process_id()
            ),
            None => String::from("Idle"),
        })
    }
}

/// Opens a WAV file, or stdin for `-`.
///
/// Inputs: The `--input` path.
/// Outputs: The input, or an error naming the path.
/// Side effects: Opens the file and reads its header.
fn open_input(input: &Path) -> Result<PcmInput, String> {
    if input == Path::new("-") {
        return Ok(PcmInput::raw(Box::new(io::stdin())));
    }
    File::open(input)
        .and_then(|file| PcmInput::wav(Box::new(io::BufReader::new(file))))
        .map_err(|error| format!("unable to read {}: {error}", input.display()))
}

/// Formats a recording summary for the terminal.
fn format_summary(summary: &RecordingSummary) -> String {
    format!(
        "recording of meeting {} finished: {} chunks transcribed, {} failed, {} segments saved",
        summary.meeting_id().as_str(),
        summary.processed_chunk_count(),
        summary.failed_chunk_count(),
        summary.segment_count()
    )
}
