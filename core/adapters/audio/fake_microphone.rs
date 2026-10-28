//! A microphone that tests drive by hand: it counts opens and closes and lets the test send
//! blocks through the open stream. Test-only.

use std::sync::{Arc, Mutex, MutexGuard, mpsc::Sender};

use crate::{
    domain::errors::AudioError,
    ports::audio::{MicrophoneSource, MicrophoneStream},
};

/// What happened to the fake microphone.
#[derive(Default)]
pub struct FakeMicrophoneLog {
    /// Successful opens.
    pub open_count: usize,
    /// Closes.
    pub close_count: usize,
    /// The open stream's sender.
    sender: Option<Sender<Vec<i16>>>,
}

/// A fake microphone source and the handle tests inspect and speak through.
#[derive(Clone)]
pub struct FakeMicrophone {
    log: Arc<Mutex<FakeMicrophoneLog>>,
    fails: bool,
}

impl FakeMicrophone {
    /// A microphone that opens.
    pub fn working() -> Self {
        Self {
            log: Arc::default(),
            fails: false,
        }
    }

    /// A microphone whose open fails with `Unavailable`.
    pub fn failing() -> Self {
        Self {
            log: Arc::default(),
            fails: true,
        }
    }

    /// The log, for assertions.
    pub fn log(&self) -> MutexGuard<'_, FakeMicrophoneLog> {
        self.log.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// `(opens, closes)` so far, read under one lock.
    pub fn counts(&self) -> (usize, usize) {
        let log = self.log();
        (log.open_count, log.close_count)
    }

    /// Sends `samples` through the open stream; nothing happens while it is closed.
    pub fn speak(&self, samples: Vec<i16>) {
        if let Some(sender) = &self.log().sender {
            let _ = sender.send(samples);
        }
    }
}

/// Closes the fake stream by dropping its sender.
struct FakeStream {
    log: Arc<Mutex<FakeMicrophoneLog>>,
}

impl MicrophoneSource for FakeMicrophone {
    fn open(&self, frames: Sender<Vec<i16>>) -> Result<Box<dyn MicrophoneStream>, AudioError> {
        if self.fails {
            return Err(AudioError::Unavailable {
                reason: String::from("no input device"),
            });
        }
        let mut log = self.log();
        log.open_count += 1;
        log.sender = Some(frames);
        Ok(Box::new(FakeStream {
            log: Arc::clone(&self.log),
        }))
    }
}

impl MicrophoneStream for FakeStream {
    fn close(self: Box<Self>) {
        let mut log = self.log.lock().unwrap_or_else(|error| error.into_inner());
        log.close_count += 1;
        log.sender = None;
    }
}
