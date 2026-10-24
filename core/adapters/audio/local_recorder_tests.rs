//! Defines the local recorder's state-machine contract before its implementation exists.

use std::sync::mpsc::TryRecvError;

use crate::{
    adapters::audio::local_recorder::LocalRecorder,
    domain::{
        errors::{AudioAction, AudioError},
        models::{AudioChunk, AudioConfig, AudioState},
    },
    ports::audio::AudioCapture,
};

/// Creates a recorder configured for deterministic, mono PCM test chunks.
fn recorder_under_test() -> LocalRecorder {
    LocalRecorder::new(AudioConfig::new(16_000, 1))
        .unwrap_or_else(|error| panic!("test recorder configuration must be valid: {error}"))
}

/// Verifies that starting an idle recorder publishes its new recording state.
#[test]
fn start_transitions_idle_recorder_to_recording() {
    let mut local_recorder = recorder_under_test();
    let state_receiver = local_recorder
        .subscribe_states()
        .unwrap_or_else(|error| panic!("state subscription must succeed: {error}"));

    local_recorder
        .start()
        .unwrap_or_else(|error| panic!("idle recorder must start: {error}"));

    assert_eq!(local_recorder.state(), AudioState::Recording);
    assert_eq!(
        state_receiver
            .recv()
            .unwrap_or_else(|error| panic!("recording state must be published: {error}")),
        AudioState::Recording
    );
}

/// Verifies that a paused recorder resumes through the start command.
#[test]
fn start_resumes_paused_recorder() {
    let mut local_recorder = recorder_under_test();
    local_recorder
        .start()
        .unwrap_or_else(|error| panic!("idle recorder must start: {error}"));
    local_recorder
        .pause()
        .unwrap_or_else(|error| panic!("recording recorder must pause: {error}"));

    local_recorder
        .start()
        .unwrap_or_else(|error| panic!("paused recorder must resume: {error}"));

    assert_eq!(local_recorder.state(), AudioState::Recording);
}

/// Verifies that pausing while idle reports the portable invalid-state failure.
#[test]
fn pause_while_idle_returns_invalid_state_error() {
    let mut local_recorder = recorder_under_test();

    let received_error = local_recorder
        .pause()
        .expect_err("an idle recorder cannot be paused");

    assert_eq!(
        received_error,
        AudioError::InvalidState {
            action: AudioAction::Pause,
            current_state: AudioState::Idle,
        }
    );
}

/// Verifies that a second start cannot replace an active recording stream.
#[test]
fn start_while_recording_returns_invalid_state_error() {
    let mut local_recorder = recorder_under_test();
    local_recorder
        .start()
        .unwrap_or_else(|error| panic!("idle recorder must start: {error}"));

    let received_error = local_recorder
        .start()
        .expect_err("an active recorder cannot start again");

    assert_eq!(
        received_error,
        AudioError::InvalidState {
            action: AudioAction::Start,
            current_state: AudioState::Recording,
        }
    );
}

/// Verifies that stopping is terminal for a recorder instance and emits the stopped state.
#[test]
fn stopped_recorder_cannot_be_restarted() {
    let mut local_recorder = recorder_under_test();
    local_recorder
        .start()
        .unwrap_or_else(|error| panic!("idle recorder must start: {error}"));
    local_recorder
        .stop()
        .unwrap_or_else(|error| panic!("recording recorder must stop: {error}"));

    let received_error = local_recorder
        .start()
        .expect_err("a stopped recorder must not restart");

    assert_eq!(local_recorder.state(), AudioState::Stopped);
    assert_eq!(
        received_error,
        AudioError::InvalidState {
            action: AudioAction::Start,
            current_state: AudioState::Stopped,
        }
    );
}

/// Verifies that chunks are delivered only while the state machine is recording.
#[test]
fn routes_audio_chunks_only_while_recording() {
    let mut local_recorder = recorder_under_test();
    let audio_receiver = local_recorder
        .subscribe_audio()
        .unwrap_or_else(|error| panic!("audio subscription must succeed: {error}"));
    let audio_chunk = AudioChunk::new(vec![12, -24], AudioConfig::new(16_000, 1));

    local_recorder
        .push_test_chunk(audio_chunk.clone())
        .unwrap_or_else(|error| panic!("idle chunks may be discarded safely: {error}"));
    assert_eq!(audio_receiver.try_recv(), Err(TryRecvError::Empty));

    local_recorder
        .start()
        .unwrap_or_else(|error| panic!("idle recorder must start: {error}"));
    local_recorder
        .push_test_chunk(audio_chunk.clone())
        .unwrap_or_else(|error| panic!("recording chunks must route: {error}"));
    assert_eq!(
        audio_receiver
            .recv()
            .unwrap_or_else(|error| panic!("recording chunk must be published: {error}")),
        audio_chunk
    );

    local_recorder
        .pause()
        .unwrap_or_else(|error| panic!("recording recorder must pause: {error}"));
    local_recorder
        .push_test_chunk(AudioChunk::new(vec![36], AudioConfig::new(16_000, 1)))
        .unwrap_or_else(|error| panic!("paused chunks may be discarded safely: {error}"));
    assert_eq!(audio_receiver.try_recv(), Err(TryRecvError::Empty));
}
