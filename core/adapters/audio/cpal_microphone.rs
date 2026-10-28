//! Captures the platform's default microphone with `cpal` (Issue #45, `microphone` feature).
//!
//! `cpal` uses ALSA on Linux (PipeWire and PulseAudio through their ALSA plugins), WASAPI on
//! Windows, and AAudio on Android. The device opens in its own default format, and
//! `PcmResampler` converts each buffer to 16 kHz mono PCM16 on the audio callback thread.
//!
//! `cpal` streams are not `Send` on every platform, so each stream lives on a dedicated thread.
//! That thread builds the stream, reports whether it started, and drops it when the stream is
//! closed. Android needs the `RECORD_AUDIO` permission first, which the app requests from Kotlin.
//! Audio is never logged; errors name the failing step only.

use std::{
    sync::mpsc::{self, Sender, SyncSender},
    thread::{self, JoinHandle},
};

use cpal::{
    SampleFormat,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

use crate::{
    adapters::audio::pcm_resampler::PcmResampler,
    domain::errors::AudioError,
    ports::audio::{MicrophoneSource, MicrophoneStream},
};

/// The default input device of the platform's default audio host.
#[derive(Clone, Copy, Debug, Default)]
pub struct CpalMicrophone;

impl CpalMicrophone {
    /// Creates the source; nothing is opened until `open`.
    pub fn new() -> Self {
        Self
    }
}

/// An open `cpal` stream, owned by its thread.
struct CpalStream {
    /// Dropping or signalling it ends the thread, which drops the stream.
    stop: SyncSender<()>,
    thread: Option<JoinHandle<()>>,
}

impl MicrophoneSource for CpalMicrophone {
    /// Opens the default input device on a new thread.
    ///
    /// Inputs: The channel for converted blocks.
    /// Outputs: The open stream, or `Unavailable` naming the step that failed (no device,
    /// unreadable format, unsupported sample format, or the stream refused to start).
    /// Side effects: Spawns the `microphone` thread and turns the microphone on.
    fn open(&self, frames: Sender<Vec<i16>>) -> Result<Box<dyn MicrophoneStream>, AudioError> {
        let (ready_sender, ready_receiver) = mpsc::sync_channel::<Result<(), String>>(1);
        let (stop_sender, stop_receiver) = mpsc::sync_channel::<()>(1);
        let thread = thread::Builder::new()
            .name(String::from("microphone"))
            .spawn(move || {
                let stream = match start_default_input(frames) {
                    Ok(stream) => stream,
                    Err(reason) => {
                        let _ = ready_sender.send(Err(reason));
                        return;
                    }
                };
                let _ = ready_sender.send(Ok(()));
                // Returns on `close` or when the stream handle is dropped.
                let _ = stop_receiver.recv();
                drop(stream);
            })
            .map_err(|error| unavailable(format!("unable to start the capture thread: {error}")))?;
        let started = ready_receiver
            .recv()
            .unwrap_or_else(|_| Err(String::from("the capture thread ended early")));
        match started {
            Ok(()) => Ok(Box::new(CpalStream {
                stop: stop_sender,
                thread: Some(thread),
            })),
            Err(reason) => {
                let _ = thread.join();
                Err(unavailable(reason))
            }
        }
    }
}

impl MicrophoneStream for CpalStream {
    /// Stops the stream and waits for its thread, so no block is sent afterwards.
    fn close(mut self: Box<Self>) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("The microphone thread panicked while closing");
        }
    }
}

/// Builds and starts a stream on the default input device, converting into `frames`.
///
/// Inputs: The channel for converted blocks; it moves into the audio callback.
/// Outputs: The playing stream, or a privacy-safe reason.
/// Side effects: Turns the microphone on.
fn start_default_input(frames: Sender<Vec<i16>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| String::from("no microphone is available"))?;
    let supported = device
        .default_input_config()
        .map_err(|error| format!("unable to read the microphone format: {error}"))?;
    let sample_format = supported.sample_format();
    let mut resampler = PcmResampler::new(supported.sample_rate(), supported.channels());
    let config: cpal::StreamConfig = supported.into();
    let on_error = |error: cpal::Error| {
        tracing::warn!(error = %error, "The microphone stream reported an error");
    };
    // A closed receiver means the recording ended; later blocks are simply dropped.
    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &_| send_block(&frames, resampler.push_f32(data)),
            on_error,
            None,
        ),
        SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &_| send_block(&frames, resampler.push_i16(data)),
            on_error,
            None,
        ),
        SampleFormat::I32 => device.build_input_stream(
            config,
            move |data: &[i32], _: &_| send_block(&frames, resampler.push_i32(data)),
            on_error,
            None,
        ),
        SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _: &_| send_block(&frames, resampler.push_u16(data)),
            on_error,
            None,
        ),
        other => return Err(format!("unsupported microphone sample format {other}")),
    }
    .map_err(|error| format!("unable to open the microphone: {error}"))?;
    stream
        .play()
        .map_err(|error| format!("unable to start the microphone: {error}"))?;
    Ok(stream)
}

/// Sends a converted block unless it is empty.
fn send_block(frames: &Sender<Vec<i16>>, block: Vec<i16>) {
    if !block.is_empty() {
        let _ = frames.send(block);
    }
}

/// Wraps a reason as the port's error.
fn unavailable(reason: String) -> AudioError {
    AudioError::Unavailable { reason }
}
