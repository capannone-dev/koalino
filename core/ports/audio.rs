//! Declares audio-capture capabilities without coupling use cases to OS audio APIs or P2P transport.

use std::sync::mpsc::Receiver;

use crate::domain::{
    errors::AudioError,
    models::{AudioChunk, AudioState},
};

/// Captures portable PCM chunks and publishes lifecycle changes to application consumers.
///
/// Local device capture and future P2P streaming adapters implement this same boundary. Consumers
/// depend only on chunks, state events, and portable errors, never on an OS-specific stream type.
pub trait AudioCapture {
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
