//! Holds an open microphone stream together with the frames it has delivered (Issue #45).
//!
//! The FFI runtime and the CLI own one while a recording captures from the microphone. They
//! drain frames into the `RecordingController` and close it on pause, interruption, or stop;
//! closing returns the frames still queued, so the last audio before a stop is never lost.
//! Audio is never logged.

use std::{
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    time::Duration,
};

use crate::{
    domain::errors::AudioError,
    ports::audio::{MicrophoneSource, MicrophoneStream},
};

/// An open microphone and its queue of 16 kHz mono PCM16 blocks.
pub struct LiveMicrophone {
    stream: Box<dyn MicrophoneStream>,
    frames: Receiver<Vec<i16>>,
}

impl LiveMicrophone {
    /// Opens `source`'s default microphone.
    ///
    /// Inputs: The microphone source.
    /// Outputs: The open microphone, or the source's error (nothing stays open then).
    /// Side effects: Turns the microphone on.
    pub fn open(source: &dyn MicrophoneSource) -> Result<Self, AudioError> {
        let (sender, frames) = mpsc::channel();
        let stream = source.open(sender)?;
        Ok(Self { stream, frames })
    }

    /// Takes every block delivered so far, without waiting.
    ///
    /// Outputs: The blocks in capture order (empty when none arrived).
    /// Side effects: Removes them from the queue.
    pub fn drain(&self) -> Vec<Vec<i16>> {
        self.frames.try_iter().collect()
    }

    /// Waits up to `timeout` for audio, then takes everything delivered.
    ///
    /// Inputs: The longest time to wait for the first block.
    /// Outputs: The blocks in capture order (empty after a timeout or a closed stream).
    /// Side effects: Blocks the calling thread up to `timeout`; removes the blocks returned.
    pub fn wait_for_frames(&self, timeout: Duration) -> Vec<Vec<i16>> {
        match self.frames.recv_timeout(timeout) {
            Ok(first_block) => {
                let mut blocks = vec![first_block];
                blocks.extend(self.frames.try_iter());
                blocks
            }
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => Vec::new(),
        }
    }

    /// Turns the microphone off and returns the blocks still queued.
    ///
    /// Outputs: The remaining blocks in capture order.
    /// Side effects: Closes the stream; waits until it sends nothing more.
    pub fn close(self) -> Vec<Vec<i16>> {
        self.stream.close();
        self.frames.try_iter().collect()
    }
}
