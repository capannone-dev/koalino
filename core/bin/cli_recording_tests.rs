//! Specifies foreground CLI recordings and their cross-process control files, with a mock
//! transcription engine and in-memory storage. Files live in unique temporary directories.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::Duration,
};

use uuid::Uuid;

use crate::{
    adapters::{
        audio::chunk_audio_store::{ChunkAudioStore, encode_wav_pcm16},
        storage::sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, pipeline::pipeline_factory::PipelineFactory,
        recording_manager::RecordingManager,
    },
    cli::RecordingControl,
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, DeviceId, MeetingId, Transcript, TranscriptionContext},
    },
    ports::ai::TranscriptionEngine,
    recording_controller::{ChunkingMode, RecordingController},
};

use super::{PipelineRecordingControl, RecordingControlFiles};

/// Returns a fixed transcript for every chunk.
struct FixedTranscriptionEngine;

impl TranscriptionEngine for FixedTranscriptionEngine {
    /// Returns "spoken words" for the recording's meeting.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        _transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            "spoken words",
        ))
    }
}

/// Builds a time-chunked controller over in-memory storage.
fn test_controller() -> anyhow::Result<RecordingController> {
    let recording_manager = RecordingManager::spawn(|| -> anyhow::Result<_> {
        Ok(PipelineFactory::new(
            FixedTranscriptionEngine,
            SqliteNoteRepository::open_in_memory(ephemeral_cipher())?,
        ))
    })?;
    Ok(RecordingController::new(
        recording_manager,
        Arc::new(ChunkAudioStore::new()),
        ChunkingMode::TimeBased,
        DeviceId::new("cli-test"),
    ))
}

/// Creates a unique directory and returns the control files of a database inside it.
fn control_files_in_temp_directory() -> (RecordingControlFiles, PathBuf) {
    let directory =
        std::env::temp_dir().join(format!("note-taker-cli-recording-{}", Uuid::new_v4()));
    fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("test directory must be created: {error}"));
    (
        RecordingControlFiles::for_database(&directory.join("notes.sqlite3")),
        directory,
    )
}

/// Builds a recording control over the test controller, without a Ctrl+C handler.
fn recording_control(
    control_files: RecordingControlFiles,
    interrupt_requested: Arc<AtomicBool>,
) -> PipelineRecordingControl {
    PipelineRecordingControl::new(control_files, Box::new(test_controller))
        .with_interrupt_flag(interrupt_requested)
        .with_stop_poll_interval(Duration::from_millis(10))
}

/// Writes `seconds` of 16 kHz mono PCM as a WAV file in `directory`.
fn write_wav(directory: &Path, seconds: usize) -> PathBuf {
    let wav_path = directory.join("input.wav");
    fs::write(
        &wav_path,
        encode_wav_pcm16(&vec![50; seconds * 16_000], 16_000),
    )
    .unwrap_or_else(|error| panic!("WAV fixture must be written: {error}"));
    wav_path
}

/// Verifies the active-recording lock is exclusive, describes its meeting, and is released.
#[test]
fn the_active_recording_lock_is_exclusive_and_released_on_drop() {
    let (control_files, directory) = control_files_in_temp_directory();

    let lock = control_files
        .acquire(&MeetingId::new("standup"))
        .unwrap_or_else(|error| panic!("first lock must be acquired: {error}"));
    let active = control_files
        .active_recording()
        .unwrap_or_else(|| panic!("the lock must be visible"));
    assert_eq!(active.meeting_id(), "standup");
    assert_eq!(active.process_id(), std::process::id());
    assert!(control_files.acquire(&MeetingId::new("other")).is_err());

    drop(lock);
    assert!(control_files.active_recording().is_none());
    let _ = fs::remove_dir_all(&directory);
}

/// Verifies a lock left by a process that no longer exists does not block a new recording.
#[cfg(target_os = "linux")]
#[test]
fn a_stale_lock_is_replaced() {
    let (control_files, directory) = control_files_in_temp_directory();
    fs::write(
        control_files.active_path(),
        format!("{}\nold-meeting\n", u32::MAX),
    )
    .unwrap_or_else(|error| panic!("stale lock must be written: {error}"));

    assert!(control_files.active_recording().is_none());
    let lock = control_files.acquire(&MeetingId::new("fresh"));
    assert!(lock.is_ok());
    drop(lock);
    let _ = fs::remove_dir_all(&directory);
}

/// Verifies status reflects the lock and stop fails when nothing is recording.
#[test]
fn status_and_stop_without_a_recording() {
    let (control_files, directory) = control_files_in_temp_directory();
    let mut control = recording_control(control_files, Arc::new(AtomicBool::new(false)));

    assert_eq!(control.status(), Ok(String::from("Idle")));
    assert!(control.request_stop().is_err());
    let _ = fs::remove_dir_all(&directory);
}

/// Verifies `record stop` signals the recording process and waits until it has finished.
#[test]
fn request_stop_waits_for_the_recording_to_finish() {
    let (control_files, directory) = control_files_in_temp_directory();
    let lock = control_files
        .acquire(&MeetingId::new("long-meeting"))
        .unwrap_or_else(|error| panic!("lock must be acquired: {error}"));
    let recorder_files = control_files.clone();
    let recording_process = thread::spawn(move || {
        while !recorder_files.stop_requested() {
            thread::sleep(Duration::from_millis(5));
        }
        thread::sleep(Duration::from_millis(30));
        drop(lock);
    });

    let mut control = recording_control(control_files.clone(), Arc::new(AtomicBool::new(false)));
    let output = control
        .request_stop()
        .unwrap_or_else(|error| panic!("stop must succeed: {error}"));

    assert!(output.contains("stopped"), "{output}");
    assert!(control_files.active_recording().is_none());
    assert!(!control_files.stop_requested());
    let _ = recording_process.join();
    let _ = fs::remove_dir_all(&directory);
}

/// Verifies a WAV recording is chunked, transcribed, summarized, and leaves no control files.
#[test]
fn recording_a_wav_file_transcribes_every_chunk() {
    let (control_files, directory) = control_files_in_temp_directory();
    let wav_path = write_wav(&directory, 31);
    let mut control = recording_control(control_files.clone(), Arc::new(AtomicBool::new(false)));

    let output = control
        .record(Some(MeetingId::new("wav-meeting")), &wav_path)
        .unwrap_or_else(|error| panic!("recording must succeed: {error}"));

    assert!(output.contains("wav-meeting"), "{output}");
    assert!(output.contains("2 chunks transcribed"), "{output}");
    assert!(output.contains("0 failed"), "{output}");
    assert!(control_files.active_recording().is_none());
    let _ = fs::remove_dir_all(&directory);
}

/// Verifies Ctrl+C (the interrupt flag) ends the recording gracefully before the input ends.
#[test]
fn an_interrupt_stops_the_recording_gracefully() {
    let (control_files, directory) = control_files_in_temp_directory();
    let wav_path = write_wav(&directory, 31);
    let interrupt_requested = Arc::new(AtomicBool::new(true));
    let mut control = recording_control(control_files.clone(), Arc::clone(&interrupt_requested));

    let output = control
        .record(Some(MeetingId::new("interrupted")), &wav_path)
        .unwrap_or_else(|error| panic!("recording must stop cleanly: {error}"));

    assert!(output.contains("0 chunks transcribed"), "{output}");
    assert!(control_files.active_recording().is_none());
    let _ = fs::remove_dir_all(&directory);
}

/// Verifies a missing input fails before any lock or pipeline is created.
#[test]
fn a_missing_input_is_reported() {
    let (control_files, directory) = control_files_in_temp_directory();
    let mut control = recording_control(control_files.clone(), Arc::new(AtomicBool::new(false)));

    let error = control
        .record(None, &directory.join("missing.wav"))
        .err()
        .unwrap_or_else(|| panic!("a missing file must fail"));

    assert!(error.contains("missing.wav"), "{error}");
    assert!(control_files.active_recording().is_none());
    let _ = fs::remove_dir_all(&directory);
}

/// Verifies a second recording is refused while one is active.
#[test]
fn a_second_recording_is_refused() {
    let (control_files, directory) = control_files_in_temp_directory();
    let wav_path = write_wav(&directory, 1);
    let _lock = control_files
        .acquire(&MeetingId::new("first"))
        .unwrap_or_else(|error| panic!("lock must be acquired: {error}"));
    let mut control = recording_control(control_files.clone(), Arc::new(AtomicBool::new(false)));

    let error = control
        .record(None, &wav_path)
        .err()
        .unwrap_or_else(|| panic!("a concurrent recording must fail"));

    assert!(error.contains("already"), "{error}");
    let _ = fs::remove_dir_all(&directory);
}
