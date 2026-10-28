//! Specifies how the FFI runtime records from the microphone (Issue #45): start opens it and its
//! audio is transcribed, stop flushes the audio still queued, pause and interruption turn it off,
//! resume turns it on again, and a microphone that cannot open rejects the start (status 7).

use std::path::PathBuf;

use super::{FFI_AUDIO_STATE_IDLE, FFI_MICROPHONE_UNAVAILABLE, FFI_SUCCESS, FfiRuntime};
use crate::{
    adapters::audio::fake_microphone::FakeMicrophone, domain::models::MeetingId,
    ports::storage::NoteRepository,
};

/// A runtime on a fresh database that records from `microphone`, and its directory.
fn runtime_with(microphone: &FakeMicrophone) -> (FfiRuntime, PathBuf) {
    let test_directory = std::env::temp_dir().join(format!(
        "note-taker-ffi-microphone-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&test_directory)
        .unwrap_or_else(|error| panic!("test directory must be created: {error}"));
    let runtime =
        FfiRuntime::open_for_tests(&test_directory.join("notes.sqlite3"), &test_directory)
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"))
            .with_microphone_source(std::sync::Arc::new(microphone.clone()));
    (runtime, test_directory)
}

/// The text of each stored segment of `meeting_id`.
fn segment_texts(runtime: &FfiRuntime, meeting_id: &str) -> Vec<String> {
    runtime
        .note_repository
        .load_transcript_segments(&MeetingId::new(meeting_id))
        .unwrap_or_else(|error| panic!("segments must load: {error}"))
        .iter()
        .map(|segment| segment.text().to_owned())
        .collect()
}

#[test]
fn a_recording_opens_the_microphone_and_its_audio_is_transcribed() {
    let microphone = FakeMicrophone::working();
    let (mut runtime, directory) = runtime_with(&microphone);

    assert_eq!(
        runtime.start_recording_for(Some(MeetingId::new("ffi-mic"))),
        FFI_SUCCESS
    );
    assert_eq!(microphone.log().open_count, 1);
    microphone.speak(vec![100; 32_000]);
    assert!(runtime.drain_microphone(), "the microphone is still open");
    assert_eq!(runtime.stop_recording(), FFI_SUCCESS);

    let texts = segment_texts(&runtime, "ffi-mic");
    drop(runtime);
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(texts, vec![String::from("transcribed chunk")]);
    assert_eq!(microphone.log().close_count, 1);
}

#[test]
fn stop_flushes_audio_still_queued_in_the_microphone() {
    let microphone = FakeMicrophone::working();
    let (mut runtime, directory) = runtime_with(&microphone);
    assert_eq!(
        runtime.start_recording_for(Some(MeetingId::new("ffi-tail"))),
        FFI_SUCCESS
    );

    microphone.speak(vec![100; 32_000]);
    assert_eq!(runtime.stop_recording(), FFI_SUCCESS);

    let texts = segment_texts(&runtime, "ffi-tail");
    drop(runtime);
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(texts.len(), 1, "the queued audio was transcribed");
}

#[test]
fn pause_and_interruption_turn_the_microphone_off_and_resume_turns_it_on() {
    let microphone = FakeMicrophone::working();
    let (mut runtime, directory) = runtime_with(&microphone);

    assert_eq!(runtime.start_recording(), FFI_SUCCESS);
    assert_eq!(runtime.pause_recording(), FFI_SUCCESS);
    let after_pause = microphone.counts();
    assert_eq!(runtime.start_recording(), FFI_SUCCESS);
    let after_resume = microphone.log().open_count;
    assert_eq!(runtime.handle_audio_interruption(), FFI_SUCCESS);
    let after_interruption = microphone.log().close_count;
    assert!(!runtime.drain_microphone(), "nothing is open to drain");

    drop(runtime);
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(after_pause, (1, 1));
    assert_eq!(after_resume, 2);
    assert_eq!(after_interruption, 2);
}

#[test]
fn a_microphone_that_cannot_open_rejects_the_start() {
    let microphone = FakeMicrophone::failing();
    let (mut runtime, directory) = runtime_with(&microphone);

    let status = runtime.start_recording();
    let state = runtime.current_recording_state();

    drop(runtime);
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(status, FFI_MICROPHONE_UNAVAILABLE);
    assert_eq!(state, FFI_AUDIO_STATE_IDLE);
}

#[test]
fn a_start_that_fails_turns_the_microphone_off_again() {
    let microphone = FakeMicrophone::working();
    let (runtime, directory) = runtime_with(&microphone);
    let mut runtime = runtime.with_recording_controller_builder(Box::new(|| {
        Err(anyhow::anyhow!("no transcription engine is configured"))
    }));

    let status = runtime.start_recording();

    drop(runtime);
    let _ = std::fs::remove_dir_all(&directory);
    assert_ne!(status, FFI_SUCCESS);
    assert_eq!(microphone.counts(), (1, 1));
}
