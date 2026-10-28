//! Declares audio-capture capabilities without coupling use cases to OS audio APIs or P2P transport.

use std::sync::mpsc::{Receiver, Sender};

use crate::domain::{
    errors::AudioError,
    models::{AudioChunk, AudioState},
};

/// Captures portable PCM chunks and publishes lifecycle changes to application consumers.
///
/// Local device capture and future P2P streaming adapters implement this same boundary. Consumers
/// depend only on chunks, state events, and portable errors, never on an OS-specific stream type.
pub trait AudioCapture {
    /// Reads the current portable recording state.
    ///
    /// Inputs: None.
    /// Outputs: The current lifecycle state without exposing platform stream types.
    /// Side effects: None.
    fn current_state(&self) -> AudioState;

    /// Starts an idle session or resumes a paused session.
    ///
    /// Inputs: None; the implementation uses configuration supplied at construction.
    /// Outputs: Success or an invalid-state/device error.
    /// Side effects: Opens or resumes adapter-managed capture and publishes the resulting state.
    fn start(&mut self) -> Result<(), AudioError>;

    /// Pauses an active session while retaining its subscriptions.
    ///
    /// Inputs: None.
    /// Outputs: Success or an invalid-state/device error.
    /// Side effects: Stops routing chunks until `start` resumes capture and publishes the new state.
    fn pause(&mut self) -> Result<(), AudioError>;

    /// Stops an active or paused session permanently.
    ///
    /// Inputs: None.
    /// Outputs: Success or an invalid-state/device error.
    /// Side effects: Closes adapter-managed capture and publishes the stopped state.
    fn stop(&mut self) -> Result<(), AudioError>;

    /// Reports an OS suspension of an active stream while retaining a recoverable session.
    ///
    /// Inputs: None; adapters call this after an OS/service interruption notification.
    /// Outputs: `AudioError::Interrupted` after entering the recoverable state, or an invalid-state error.
    /// Side effects: Stops chunk routing and publishes `AudioState::Interrupted`; `start` may resume it.
    fn handle_interruption(&mut self) -> Result<(), AudioError>;

    /// Creates an independent subscription for captured PCM chunks.
    ///
    /// Inputs: None.
    /// Outputs: A receiver of adapter-neutral chunks or a subscription error.
    /// Side effects: Registers a new consumer that receives future recording-state chunks.
    fn subscribe_audio(&mut self) -> Result<Receiver<AudioChunk>, AudioError>;

    /// Creates an independent subscription for future lifecycle transitions.
    ///
    /// Inputs: None.
    /// Outputs: A receiver of portable audio states or a subscription error.
    /// Side effects: Registers a new consumer that receives future state transitions.
    fn subscribe_states(&mut self) -> Result<Receiver<AudioState>, AudioError>;
}

/// Opens a microphone and delivers its audio as 16 kHz mono PCM16 blocks (Issue #45).
///
/// The `cpal` adapter implements it for the platform's default input device; tests use fakes.
pub trait MicrophoneSource: Send + Sync {
    /// Starts capture from the default input device.
    ///
    /// Inputs: The channel that receives each converted block, in order.
    /// Outputs: The open stream, or `AudioError::Unavailable` when there is no device, access is
    /// denied, or its format is unsupported.
    /// Side effects: Turns the microphone on (the platform's recording indicator shows) until
    /// the stream is closed.
    fn open(&self, frames: Sender<Vec<i16>>) -> Result<Box<dyn MicrophoneStream>, AudioError>;
}

/// An open microphone stream.
pub trait MicrophoneStream: Send {
    /// Stops capture.
    ///
    /// Outputs: Returns once no further block will be sent.
    /// Side effects: Turns the microphone off and drops the sending side of the channel.
    fn close(self: Box<Self>);
}
