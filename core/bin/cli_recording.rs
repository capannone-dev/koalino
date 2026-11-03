//! Runs CLI recordings in the foreground and coordinates `record stop` across processes.
//!
//! `record start` reads PCM from a WAV file or stdin (`PcmInput`), or from the default
//! microphone when no input is given (Issue #45), feeds it to a `RecordingController`, and
//! finishes when the input ends, on Ctrl+C, or when another process runs `record stop`. The
//! audio the microphone still holds at a stop is transcribed too. Processes coordinate through two files next to the database:
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
    adapters::audio::{live_microphone::LiveMicrophone, pcm_input::PcmInput},
    adapters::storage::private_files::remove_meeting_folder,
    application::deletion::MeetingDeletionHooks,
    application::recording_manager::RecordingSummary,
    cli::RecordingControl,
    domain::models::{MeetingId, MeetingTitle},
    ports::audio::MicrophoneSource,
    recording_controller::RecordingController,
};

#[cfg(test)]
#[path = "cli_recording_tests.rs"]
mod tests;

/// How long `record stop` waits for the recording process to finish transcribing.
const DEFAULT_STOP_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// How often `record stop` checks whether the recording has finished.
const DEFAULT_STOP_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// How long a microphone recording waits for audio before checking for a stop again.
const MICROPHONE_WAIT: Duration = Duration::from_millis(100);

/// Where a recording's audio comes from.
enum AudioInput {
    /// A WAV file or raw stdin PCM.
    Pcm(PcmInput),
    /// The default microphone.
    Microphone(LiveMicrophone),
}

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
    /// The microphone used when no input is given; `None` without the `microphone` feature.
    microphone_source: Option<Arc<dyn MicrophoneSource>>,
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
            microphone_source: None,
            stop_poll_interval: DEFAULT_STOP_POLL_INTERVAL,
            stop_timeout: DEFAULT_STOP_TIMEOUT,
        }
    }

    /// Records from `microphone_source` when `record` gets no input.
    ///
    /// Inputs: The microphone source.
    /// Outputs: The control using it.
    /// Side effects: None; the microphone opens only when a recording starts.
    pub fn with_microphone_source(mut self, microphone_source: Arc<dyn MicrophoneSource>) -> Self {
        self.microphone_source = Some(microphone_source);
        self
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

    /// Whether Ctrl+C or `record stop` asked the recording to finish.
    fn stop_requested(&self) -> bool {
        self.interrupt_requested.load(Ordering::SeqCst) || self.control_files.stop_requested()
    }

    /// Opens the `--input` file or stdin, or the microphone when there is none.
    ///
    /// Inputs: The `--input` path, if any.
    /// Outputs: The opened input, or an error naming what could not be opened.
    /// Side effects: Opens the file or turns the microphone on.
    fn open_audio_input(&self, input: Option<&Path>) -> Result<AudioInput, String> {
        if let Some(input) = input {
            return open_input(input).map(AudioInput::Pcm);
        }
        let Some(microphone_source) = &self.microphone_source else {
            return Err(String::from(
                "this build cannot record the microphone; pass --input with a WAV file or - for stdin",
            ));
        };
        LiveMicrophone::open(microphone_source.as_ref())
            .map(AudioInput::Microphone)
            .map_err(|error| format!("unable to open the microphone: {error}"))
    }

    /// Feeds the input to the controller until it ends or a stop is requested.
    ///
    /// Inputs: The started controller and the opened input.
    /// Outputs: `Ok(())`, or the first input or pipeline error.
    /// Side effects: Pushes PCM; chunks are transcribed in the background. A microphone is
    /// closed before returning, and the audio it still held is pushed.
    fn capture_until_stopped(
        &self,
        controller: &mut RecordingController,
        audio_input: AudioInput,
    ) -> Result<(), String> {
        match audio_input {
            AudioInput::Pcm(mut pcm_input) => self.capture_pcm(controller, &mut pcm_input),
            AudioInput::Microphone(microphone) => self.capture_microphone(controller, microphone),
        }
    }

    /// Records the microphone until a stop is requested, then pushes its queued tail.
    ///
    /// Side effects: Pushes PCM; closes the microphone.
    fn capture_microphone(
        &self,
        controller: &mut RecordingController,
        microphone: LiveMicrophone,
    ) -> Result<(), String> {
        let mut capture_result = Ok(());
        while !self.stop_requested() && capture_result.is_ok() {
            capture_result = push_blocks(controller, microphone.wait_for_frames(MICROPHONE_WAIT));
        }
        let tail = microphone.close();
        capture_result.and(push_blocks(controller, tail))
    }

    /// Feeds a file or stdin until it ends or a stop is requested.
    ///
    /// Side effects: Reads the input and pushes PCM.
    fn capture_pcm(
        &self,
        controller: &mut RecordingController,
        pcm_input: &mut PcmInput,
    ) -> Result<(), String> {
        loop {
            if self.stop_requested() {
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
    /// Inputs: The meeting (`None` generates `meeting-<uuid>`), an optional title (`None` keeps
    /// the meeting's current title), and a WAV path or `-` for raw stdin PCM (`None`: the
    /// default microphone).
    /// Outputs: A summary once every chunk is transcribed, or a readable error.
    /// Side effects: Holds the active file, composes the pipeline, stores the title, persists
    /// segments, and prints progress to stderr (never audio or transcript text).
    fn record(
        &mut self,
        meeting_id: Option<MeetingId>,
        title: Option<MeetingTitle>,
        input: Option<&Path>,
    ) -> Result<String, String> {
        let audio_input = self.open_audio_input(input)?;
        let meeting_id =
            meeting_id.unwrap_or_else(|| MeetingId::new(format!("meeting-{}", Uuid::new_v4())));
        let _lock = self
            .control_files
            .acquire(&meeting_id)
            .map_err(|error| error.to_string())?;
        let mut controller = (self.controller_builder)().map_err(|error| format!("{error:#}"))?;
        let title_label = title
            .as_ref()
            .map(|title| format!(" \"{}\"", title.as_str()))
            .unwrap_or_default();
        controller
            .start_titled(Some(meeting_id.clone()), title)
            .map_err(|error| error.to_string())?;
        if self.handles_ctrl_c {
            self.install_ctrl_c_handler();
        }
        eprintln!(
            "Recording into meeting {}{title_label}. Press Ctrl+C or run `note-taker record stop` to finish.",
            meeting_id.as_str()
        );
        let capture_result = self.capture_until_stopped(&mut controller, audio_input);
        eprintln!("Waiting for the remaining audio to be transcribed…");
        let summary = controller.stop().map_err(|error| error.to_string())?;
        capture_result?;
        Ok(format_summary(&summary, &title_label))
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

/// Answers deletion questions for the CLI and the daemon from this database's files.
pub struct LocalMeetingFiles {
    /// Shows which meeting a recording in any process is writing to.
    control_files: RecordingControlFiles,
    /// Holds each meeting's private attachment folder.
    attachment_directory: PathBuf,
}

impl LocalMeetingFiles {
    /// Locates the files of `database_path`.
    ///
    /// Inputs: The database, and `NOTE_TAKER_ATTACHMENT_DIR` when set (as P2P composition reads it).
    /// Outputs: The hooks. Side effects: Reads one environment variable.
    pub fn for_database(database_path: &Path) -> Self {
        let attachment_directory = std::env::var("NOTE_TAKER_ATTACHMENT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                database_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("attachments")
            });
        Self::new(
            RecordingControlFiles::for_database(database_path),
            attachment_directory,
        )
    }

    /// Creates hooks from explicit locations.
    pub fn new(control_files: RecordingControlFiles, attachment_directory: PathBuf) -> Self {
        Self {
            control_files,
            attachment_directory,
        }
    }

    /// The attachment folder these hooks clean, which also holds project resources (#85).
    pub fn attachment_directory(&self) -> &Path {
        &self.attachment_directory
    }
}

impl MeetingDeletionHooks for LocalMeetingFiles {
    /// Compares `meeting_id` with the active-recording file. Side effects: Reads that file.
    fn is_recording(&self, meeting_id: &MeetingId) -> bool {
        self.control_files
            .active_recording()
            .is_some_and(|active| active.meeting_id() == meeting_id.as_str())
    }

    /// Removes the meeting's attachment folder. Side effects: Deletes files; logs a failure.
    fn remove_files(&self, meeting_id: &MeetingId) {
        if remove_meeting_folder(&self.attachment_directory, meeting_id.as_str()).is_err() {
            tracing::warn!("a deleted meeting's attachment files could not be removed");
        }
    }
}

/// Pushes microphone blocks into the recording, stopping at the first pipeline error.
fn push_blocks(controller: &mut RecordingController, blocks: Vec<Vec<i16>>) -> Result<(), String> {
    for block in blocks {
        controller
            .push_pcm(block)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
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
///
/// Inputs: The summary and a ` "<title>"` label (empty for an untitled start).
/// Outputs: One summary line.
/// Side effects: None.
fn format_summary(summary: &RecordingSummary, title_label: &str) -> String {
    format!(
        "recording of meeting {}{title_label} finished: {} chunks transcribed, {} failed, {} segments saved",
        summary.meeting_id().as_str(),
        summary.processed_chunk_count(),
        summary.failed_chunk_count(),
        summary.segment_count()
    )
}
