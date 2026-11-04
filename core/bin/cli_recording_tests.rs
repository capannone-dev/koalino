//! Specifies foreground CLI recordings and their cross-process control files, with a mock
//! transcription engine and in-memory storage. Files live in unique temporary directories.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use clap::Parser;
use uuid::Uuid;

use crate::{
    adapters::{
        audio::{
            chunk_audio_store::{ChunkAudioStore, encode_wav_pcm16},
            fake_microphone::FakeMicrophone,
        },
        export::default_exporters,
        storage::{
            in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
        },
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, export_manager::ExportWriter,
        pipeline::pipeline_factory::PipelineFactory, recording_manager::RecordingManager,
    },
    cli::{
        Cli, CliCommand, CoreCliService, MeetingArgs, MeetingCommand, RecordingControl, dispatch,
    },
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, DeviceId, MeetingId, Transcript, TranscriptionContext},
        transcription_language::TranscriptionLanguage,
    },
    ports::ai::TranscriptionEngine,
    ports::storage::NoteRepository,
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
    let wav_path = write_wav(&directory, 16);
    let mut control = recording_control(control_files.clone(), Arc::new(AtomicBool::new(false)));

    let output = control
        .record(
            Some(MeetingId::new("wav-meeting")),
            None,
            Some(&wav_path),
            None,
        )
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
        .record(
            Some(MeetingId::new("interrupted")),
            None,
            Some(&wav_path),
            None,
        )
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
        .record(None, None, Some(&directory.join("missing.wav")), None)
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
        .record(None, None, Some(&wav_path), None)
        .err()
        .unwrap_or_else(|| panic!("a concurrent recording must fail"));

    assert!(error.contains("already"), "{error}");
    let _ = fs::remove_dir_all(&directory);
}

/// Opens a file-backed repository, as the CLI binary and its recording worker each do.
fn open_file_repository(database_path: &Path) -> anyhow::Result<SqliteNoteRepository> {
    Ok(SqliteNoteRepository::open(
        database_path,
        DeviceId::new("cli-test"),
        ephemeral_cipher(),
    )?)
}

/// Verifies `record start --title "Test"` creates a meeting with that title, which
/// `meetings list` then shows next to its generated ID.
#[test]
fn record_start_with_a_title_creates_a_titled_meeting() {
    let (_, directory) = control_files_in_temp_directory();
    let database_path = directory.join("notes.sqlite3");
    let wav_path = write_wav(&directory, 1);
    let worker_database_path = database_path.clone();
    let recording_control = PipelineRecordingControl::new(
        RecordingControlFiles::for_database(&database_path),
        Box::new(move || {
            let worker_database_path = worker_database_path.clone();
            let recording_manager = RecordingManager::spawn(move || -> anyhow::Result<_> {
                Ok(PipelineFactory::new(
                    FixedTranscriptionEngine,
                    open_file_repository(&worker_database_path)?,
                ))
            })?;
            Ok(RecordingController::new(
                recording_manager,
                Arc::new(ChunkAudioStore::new()),
                ChunkingMode::TimeBased,
                DeviceId::new("cli-test"),
            ))
        }),
    )
    .with_interrupt_flag(Arc::new(AtomicBool::new(false)));
    let repository = open_file_repository(&database_path)
        .unwrap_or_else(|error| panic!("repository must open: {error}"));
    let mut cli_service = CoreCliService::new(
        recording_control,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), directory.clone()),
    );
    let wav_argument = wav_path.to_string_lossy().into_owned();
    let parsed_cli = Cli::try_parse_from([
        "note-taker",
        "record",
        "start",
        "--title",
        "Test",
        "--input",
        &wav_argument,
    ])
    .unwrap_or_else(|error| panic!("record start must parse: {error}"));

    let record_output = dispatch(parsed_cli.command, &mut cli_service)
        .unwrap_or_else(|error| panic!("recording must succeed: {error}"));
    let list_output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::List,
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("meeting list must succeed: {error}"));
    drop(cli_service);
    let _ = fs::remove_dir_all(&directory);

    assert!(record_output.contains("\"Test\""), "{record_output}");
    let meeting_rows: Vec<Vec<&str>> = list_output
        .lines()
        .skip(1)
        .map(|line| line.split(" | ").collect())
        .collect();
    assert_eq!(meeting_rows.len(), 1, "{list_output}");
    assert!(meeting_rows[0][0].starts_with("meeting-"), "{list_output}");
    assert_eq!(meeting_rows[0][1], "Test");
}

/// Verifies `record start --language it` records Italian in the meeting's metadata, and that a
/// recording without `--language` records the configured default instead.
#[test]
fn record_start_records_the_chosen_or_configured_language() {
    let (_, directory) = control_files_in_temp_directory();
    let database_path = directory.join("notes.sqlite3");
    let wav_path = write_wav(&directory, 1);
    let worker_database_path = database_path.clone();
    let open_repository = |path: &Path| {
        crate::storage_composition::open_note_repository(
            path,
            DeviceId::new("cli-test"),
            ephemeral_cipher(),
        )
    };
    let recording_control = PipelineRecordingControl::new(
        RecordingControlFiles::for_database(&database_path),
        Box::new(move || {
            let worker_database_path = worker_database_path.clone();
            let recording_manager = RecordingManager::spawn(move || -> anyhow::Result<_> {
                Ok(PipelineFactory::new(
                    FixedTranscriptionEngine,
                    open_repository(&worker_database_path)?,
                ))
            })?;
            // What `NOTE_TAKER_WHISPER_LANGUAGE=de` gives the CLI.
            Ok(RecordingController::new(
                recording_manager,
                Arc::new(ChunkAudioStore::new()),
                ChunkingMode::TimeBased,
                DeviceId::new("cli-test"),
            )
            .with_default_language(TranscriptionLanguage::parse("de").ok()))
        }),
    )
    .with_interrupt_flag(Arc::new(AtomicBool::new(false)));
    let repository = open_repository(&database_path)
        .unwrap_or_else(|error| panic!("repository must open: {error}"));
    let mut cli_service = CoreCliService::new(
        recording_control,
        repository,
        InMemoryVoiceVectorDB::new(),
        ExportWriter::new(default_exporters(), directory.clone()),
    );
    let wav_argument = wav_path.to_string_lossy().into_owned();
    for (meeting_id, language_arguments) in [
        ("meeting-italian", vec!["--language", "it"]),
        ("meeting-default", vec![]),
    ] {
        let mut arguments = vec![
            "note-taker",
            "record",
            "start",
            "--meeting-id",
            meeting_id,
            "--input",
            &wav_argument,
        ];
        arguments.extend(language_arguments);
        let parsed_cli = Cli::try_parse_from(arguments)
            .unwrap_or_else(|error| panic!("record start must parse: {error}"));
        dispatch(parsed_cli.command, &mut cli_service)
            .unwrap_or_else(|error| panic!("recording must succeed: {error}"));
    }
    let view_output = dispatch(
        CliCommand::Meetings(MeetingArgs {
            command: MeetingCommand::View {
                meeting_id: String::from("meeting-italian"),
            },
        }),
        &mut cli_service,
    )
    .unwrap_or_else(|error| panic!("meeting view must succeed: {error}"));
    drop(cli_service);
    let repository = open_repository(&database_path)
        .unwrap_or_else(|error| panic!("repository must reopen: {error}"));
    let languages_of = |meeting_id: &str| -> Vec<&'static str> {
        repository
            .load_meeting_metadata(&MeetingId::new(meeting_id))
            .ok()
            .flatten()
            .unwrap_or_else(|| panic!("{meeting_id} must exist"))
            .transcription_languages()
            .iter()
            .map(|language| language.as_str())
            .collect()
    };
    let italian = languages_of("meeting-italian");
    let default = languages_of("meeting-default");
    drop(repository);
    let _ = fs::remove_dir_all(&directory);

    assert_eq!(italian, ["it"]);
    assert_eq!(default, ["de"]);
    assert!(
        view_output
            .lines()
            .any(|line| line == "Transcription language: it"),
        "{view_output}"
    );
}

/// Verifies `record start` without `--input` records the microphone until stopped, including
/// the audio still queued when the stop arrives.
#[test]
fn recording_without_an_input_captures_the_microphone() {
    let (control_files, directory) = control_files_in_temp_directory();
    let interrupt_requested = Arc::new(AtomicBool::new(false));
    let microphone = FakeMicrophone::working();
    let mut control = recording_control(control_files.clone(), Arc::clone(&interrupt_requested))
        .with_microphone_source(Arc::new(microphone.clone()));
    let speaker = microphone.clone();
    let speaker_interrupt = Arc::clone(&interrupt_requested);
    let speaker_thread = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while speaker.counts().0 == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        speaker.speak(vec![50; 32_000]);
        speaker_interrupt.store(true, Ordering::SeqCst);
    });

    let output = control.record(Some(MeetingId::new("mic-meeting")), None, None, None);
    let _ = speaker_thread.join();
    let _ = fs::remove_dir_all(&directory);

    let output = output.unwrap_or_else(|error| panic!("recording must succeed: {error}"));
    assert!(output.contains("1 chunks transcribed"), "{output}");
    assert_eq!(microphone.counts(), (1, 1));
    assert!(control_files.active_recording().is_none());
}

/// Verifies a build without microphone support asks for `--input`.
#[test]
fn recording_without_an_input_or_a_microphone_is_reported() {
    let (control_files, directory) = control_files_in_temp_directory();
    let mut control = recording_control(control_files.clone(), Arc::new(AtomicBool::new(false)));

    let error = control.record(None, None, None, None).err();
    let _ = fs::remove_dir_all(&directory);

    let error = error.unwrap_or_else(|| panic!("recording must fail without an input"));
    assert!(error.contains("--input"), "{error}");
    assert!(control_files.active_recording().is_none());
}

/// Verifies a microphone that cannot open fails before any lock or pipeline is created.
#[test]
fn a_microphone_that_cannot_open_is_reported() {
    let (control_files, directory) = control_files_in_temp_directory();
    let mut control = recording_control(control_files.clone(), Arc::new(AtomicBool::new(false)))
        .with_microphone_source(Arc::new(FakeMicrophone::failing()));

    let error = control.record(None, None, None, None).err();
    let _ = fs::remove_dir_all(&directory);

    let error = error.unwrap_or_else(|| panic!("recording must fail without a microphone"));
    assert!(error.contains("microphone"), "{error}");
    assert!(control_files.active_recording().is_none());
}
