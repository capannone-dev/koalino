//! Implements a local audio-capture state machine without exposing OS stream details to the core.

use std::sync::mpsc::{self, Receiver, Sender};

use tracing::{info, warn};

use crate::{
    domain::{
        errors::{AudioAction, AudioError},
        models::{AudioChunk, AudioConfig, AudioState},
    },
    ports::audio::AudioCapture,
};

/// Owns one local capture session and broadcasts its portable events to subscribed consumers.
///
/// This adapter deliberately has no OS audio dependency yet. A future platform stream feeds chunks
/// through the same internal routing boundary; a P2P adapter can instead implement `AudioCapture`.
pub struct LocalRecorder {
    audio_config: AudioConfig,
    current_state: AudioState,
    audio_subscribers: Vec<Sender<AudioChunk>>,
    state_subscribers: Vec<Sender<AudioState>>,
}

impl LocalRecorder {
    /// Creates an idle local recorder with the requested PCM format.
    ///
    /// Inputs: `audio_config` must specify a nonzero sample rate and channel count.
    /// Outputs: An idle recorder or a portable configuration error.
    /// Side effects: Allocates only in-memory subscriber registries; it does not open an OS device.
    pub fn new(audio_config: AudioConfig) -> Result<Self, AudioError> {
        if audio_config.sample_rate_hz() == 0 || audio_config.channels() == 0 {
            return Err(AudioError::InvalidConfiguration {
                reason: "sample rate and channel count must both be nonzero".to_owned(),
            });
        }

        Ok(Self {
            audio_config,
            current_state: AudioState::Idle,
            audio_subscribers: Vec::new(),
            state_subscribers: Vec::new(),
        })
    }

    /// Returns the recorder's current lifecycle state.
    ///
    /// Inputs: None.
    /// Outputs: The state currently enforced by the local state machine.
    /// Side effects: None.
    pub fn state(&self) -> AudioState {
        self.current_state
    }

    /// Routes a deterministic chunk through the local capture seam for tests and platform bridges.
    ///
    /// Inputs: `audio_chunk` must use the configuration supplied when this recorder was created.
    /// Outputs: Success after routing or discarding the chunk, or a configuration error.
    /// Side effects: Sends raw audio only to in-process subscribers while recording; never logs it.
    pub fn push_test_chunk(&mut self, audio_chunk: AudioChunk) -> Result<(), AudioError> {
        if audio_chunk.config() != self.audio_config {
            return Err(AudioError::InvalidConfiguration {
                reason: "audio chunk format does not match the recorder configuration".to_owned(),
            });
        }

        if self.current_state == AudioState::Recording {
            Self::publish_to_subscribers(&mut self.audio_subscribers, audio_chunk);
        }

        Ok(())
    }

    /// Marks an active stream as OS-interrupted while retaining it for a later safe resume.
    ///
    /// Inputs: None; a platform service calls this after its audio stream is suspended.
    /// Outputs: `AudioError::Interrupted` after transitioning from recording, or an invalid-state
    /// error when no active stream can be interrupted.
    /// Side effects: Stops chunk routing, emits a structured warning, and publishes Interrupted.
    fn transition_to_interrupted(&mut self) -> Result<(), AudioError> {
        match self.current_state {
            AudioState::Recording => {
                self.current_state = AudioState::Interrupted;
                warn!("Audio stream interrupted by OS, entering recoverable state");
                Self::publish_to_subscribers(&mut self.state_subscribers, AudioState::Interrupted);
                Err(AudioError::Interrupted)
            }
            AudioState::Interrupted => {
                warn!("Audio stream interruption repeated while already recoverable");
                Err(AudioError::Interrupted)
            }
            current_state => Err(AudioError::InvalidState {
                action: AudioAction::Interrupt,
                current_state,
            }),
        }
    }

    /// Applies a validated lifecycle transition and publishes it without exposing audio payloads.
    ///
    /// Inputs: `action` identifies the requested command and `next_state` is its permitted target.
    /// Outputs: Success or an `AudioError::InvalidState` describing the rejected command.
    /// Side effects: Updates local lifecycle state, emits a structured transition log, and notifies subscribers.
    fn transition(
        &mut self,
        action: AudioAction,
        next_state: AudioState,
    ) -> Result<(), AudioError> {
        if !Self::is_transition_allowed(self.current_state, action) {
            return Err(AudioError::InvalidState {
                action,
                current_state: self.current_state,
            });
        }

        self.current_state = next_state;
        info!(state = ?next_state, "Audio capture state changed");
        Self::publish_to_subscribers(&mut self.state_subscribers, next_state);
        Ok(())
    }

    /// Determines whether a lifecycle command is legal from a portable audio state.
    ///
    /// Inputs: `current_state` is the present state and `action` is the requested command.
    /// Outputs: `true` when the state machine permits the transition.
    /// Side effects: None.
    fn is_transition_allowed(current_state: AudioState, action: AudioAction) -> bool {
        matches!(
            (current_state, action),
            (AudioState::Idle, AudioAction::Start)
                | (AudioState::Recording, AudioAction::Pause)
                | (AudioState::Recording, AudioAction::Stop)
                | (AudioState::Paused, AudioAction::Start)
                | (AudioState::Paused, AudioAction::Stop)
                | (AudioState::Interrupted, AudioAction::Start)
                | (AudioState::Interrupted, AudioAction::Stop)
        )
    }

    /// Broadcasts one event and removes subscribers whose receivers have disconnected.
    ///
    /// Inputs: `subscribers` is the registered output list and `event` is cloned for each receiver.
    /// Outputs: None.
    /// Side effects: Sends to live receivers and removes disconnected receiver senders.
    fn publish_to_subscribers<Event>(subscribers: &mut Vec<Sender<Event>>, event: Event)
    where
        Event: Clone,
    {
        subscribers.retain(|subscriber| subscriber.send(event.clone()).is_ok());
    }
}

impl AudioCapture for LocalRecorder {
    /// Starts this local recorder from idle or resumes it from paused.
    ///
    /// Inputs: None.
    /// Outputs: Success or a portable invalid-state error.
    /// Side effects: Changes state and notifies state subscribers; no OS device is opened in this scaffold.
    fn start(&mut self) -> Result<(), AudioError> {
        self.transition(AudioAction::Start, AudioState::Recording)
    }

    /// Pauses a currently recording local recorder.
    ///
    /// Inputs: None.
    /// Outputs: Success or a portable invalid-state error.
    /// Side effects: Changes state and prevents new chunks from being routed until resumed.
    fn pause(&mut self) -> Result<(), AudioError> {
        self.transition(AudioAction::Pause, AudioState::Paused)
    }

    /// Stops this local recorder permanently.
    ///
    /// Inputs: None.
    /// Outputs: Success or a portable invalid-state error.
    /// Side effects: Changes state, prevents future chunk routing, and notifies state subscribers.
    fn stop(&mut self) -> Result<(), AudioError> {
        self.transition(AudioAction::Stop, AudioState::Stopped)
    }

    /// Converts an OS audio suspension into a recoverable portable interruption state.
    ///
    /// Inputs: None; callers invoke this only after an adapter detects stream suspension.
    /// Outputs: `AudioError::Interrupted` once the session is recoverable, or invalid-state error.
    /// Side effects: Stops chunk routing and notifies state subscribers without destroying session state.
    fn handle_interruption(&mut self) -> Result<(), AudioError> {
        self.transition_to_interrupted()
    }

    /// Subscribes to future chunks routed while the recorder is recording.
    ///
    /// Inputs: None.
    /// Outputs: A new audio receiver.
    /// Side effects: Registers a new in-memory subscriber.
    fn subscribe_audio(&mut self) -> Result<Receiver<AudioChunk>, AudioError> {
        let (audio_sender, audio_receiver) = mpsc::channel();
        self.audio_subscribers.push(audio_sender);
        Ok(audio_receiver)
    }

    /// Subscribes to future lifecycle state transitions.
    ///
    /// Inputs: None.
    /// Outputs: A new state receiver.
    /// Side effects: Registers a new in-memory subscriber.
    fn subscribe_states(&mut self) -> Result<Receiver<AudioState>, AudioError> {
        let (state_sender, state_receiver) = mpsc::channel();
        self.state_subscribers.push(state_sender);
        Ok(state_receiver)
    }
}
