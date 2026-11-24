//! Captures the platform's default microphone with `cpal` (Issue #45, `microphone` feature).
//!
//! `cpal` uses ALSA on Linux (PipeWire and PulseAudio through their ALSA plugins), WASAPI on
//! Windows, and AAudio on Android. On Linux, when ALSA has no usable `default` input, the
//! microphone falls back to the sound server and to ALSA's `pipewire`/`pulse` devices (#152). The device opens in its own default format, and
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
    adapters::audio::{
        capture_error_throttle::CaptureErrorThrottle,
        microphone_fallback::{MICROPHONE_FIX_HINT, MicrophoneAttempt, open_first},
        pcm_resampler::PcmResampler,
    },
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
        open_on_thread("microphone", move || start_default_input(frames))
    }
}

/// Builds and plays a capture stream on its own thread, which owns it until it is closed.
///
/// Inputs: The thread's name, and the function that builds and starts the stream there.
/// Outputs: The open stream, or `Unavailable` with the builder's reason.
/// Side effects: Spawns the thread; the stream captures until `close`.
pub(crate) fn open_on_thread(
    thread_name: &str,
    build: impl FnOnce() -> Result<cpal::Stream, String> + Send + 'static,
) -> Result<Box<dyn MicrophoneStream>, AudioError> {
    let (ready_sender, ready_receiver) = mpsc::sync_channel::<Result<(), String>>(1);
    let (stop_sender, stop_receiver) = mpsc::sync_channel::<()>(1);
    let thread = thread::Builder::new()
        .name(thread_name.to_owned())
        .spawn(move || {
            let stream = match build() {
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

/// Builds and starts a stream on the default microphone, converting into `frames`.
///
/// On Linux it tries, in order: ALSA's `default` device (as before #152); the sound server's
/// default source through cpal's PulseAudio host, which PipeWire also serves (only in builds with
/// `computer-sound`, which compiles that host in); then ALSA's `pipewire` and `pulse` devices.
/// Elsewhere it opens the platform's default input.
///
/// Inputs: The channel for converted blocks; it moves into the audio callback of the attempt that
/// opens.
/// Outputs: The playing stream, or a privacy-safe reason naming every attempt (and, on Linux, how
/// to give ALSA a default microphone).
/// Side effects: Turns the microphone on. A fallback, or a failure of every attempt, is logged.
fn start_default_input(frames: Sender<Vec<i16>>) -> Result<cpal::Stream, String> {
    let attempts = microphone_attempts(&frames);
    let hint = if cfg!(target_os = "linux") {
        MICROPHONE_FIX_HINT
    } else {
        ""
    };
    match open_first(attempts, hint) {
        Ok((stream, _label)) => Ok(stream),
        Err(reason) => {
            // The app only says it could not open the microphone; the reason is in the log.
            tracing::error!(reason = %reason, "The microphone could not be opened");
            Err(reason)
        }
    }
}

/// The ways to open the microphone on Linux, most preferred first (#152).
#[cfg(target_os = "linux")]
fn microphone_attempts(frames: &Sender<Vec<i16>>) -> Vec<MicrophoneAttempt<'static, cpal::Stream>> {
    let mut attempts: Vec<MicrophoneAttempt<'static, cpal::Stream>> = Vec::new();
    let alsa_default_frames = frames.clone();
    attempts.push((
        "ALSA default",
        Box::new(move || {
            let host = cpal::host_from_id(cpal::HostId::Alsa)
                .map_err(|error| format!("ALSA is not available: {error}"))?;
            let device = host
                .default_input_device()
                .ok_or_else(|| String::from("no default input device"))?;
            start_on_device(&device, alsa_default_frames, None)
        }),
    ));
    #[cfg(feature = "computer-sound")]
    {
        let server_frames = frames.clone();
        attempts.push((
            "sound server (PulseAudio/PipeWire)",
            Box::new(move || {
                let host = cpal::host_from_id(cpal::HostId::PulseAudio)
                    .map_err(|error| format!("the sound server is not available: {error}"))?;
                let device = host
                    .default_input_device()
                    .ok_or_else(|| String::from("no default source"))?;
                let supported = device
                    .default_input_config()
                    .map_err(|error| format!("unable to read the microphone format: {error}"))?;
                // PulseAudio's default fragments are about 2 s; ask for 50 ms like the computer's
                // sound (#108), so the live transcript and the echo check keep their timing.
                let (min_frames, max_frames) = match supported.buffer_size() {
                    cpal::SupportedBufferSize::Range { min, max } => (*min, *max),
                    cpal::SupportedBufferSize::Unknown => (1, u32::MAX),
                };
                let buffer_frames =
                    crate::adapters::audio::cpal_computer_sound::monitor_buffer_frames(
                        supported.sample_rate(),
                        min_frames,
                        max_frames,
                    );
                start_capture(
                    "microphone",
                    &device,
                    supported,
                    server_frames,
                    Some(buffer_frames),
                )
            }),
        ));
    }
    for (label, pcm_name) in [("ALSA pipewire", "pipewire"), ("ALSA pulse", "pulse")] {
        let pcm_frames = frames.clone();
        attempts.push((
            label,
            Box::new(move || {
                let host = cpal::host_from_id(cpal::HostId::Alsa)
                    .map_err(|error| format!("ALSA is not available: {error}"))?;
                let device = host
                    .device_by_id(&cpal::DeviceId::new(cpal::HostId::Alsa, pcm_name))
                    .ok_or_else(|| format!("ALSA has no `{pcm_name}` device"))?;
                start_on_device(&device, pcm_frames, None)
            }),
        ));
    }
    attempts
}

/// The way to open the microphone elsewhere: the platform's default input.
#[cfg(not(target_os = "linux"))]
fn microphone_attempts(frames: &Sender<Vec<i16>>) -> Vec<MicrophoneAttempt<'static, cpal::Stream>> {
    let default_frames = frames.clone();
    vec![(
        "default input",
        Box::new(move || {
            let device = cpal::default_host()
                .default_input_device()
                .ok_or_else(|| String::from("no microphone is available"))?;
            start_on_device(&device, default_frames, None)
        }),
    )]
}

/// Opens `device` in its default input format and starts capturing into `frames`.
///
/// Inputs: The device, the channel for converted blocks, and the frames per block (`None` for the
/// host's default). Outputs: The playing stream, or a privacy-safe reason.
/// Side effects: Turns the microphone on.
fn start_on_device(
    device: &cpal::Device,
    frames: Sender<Vec<i16>>,
    buffer_frames: Option<u32>,
) -> Result<cpal::Stream, String> {
    let supported = device
        .default_input_config()
        .map_err(|error| format!("unable to read the microphone format: {error}"))?;
    start_capture("microphone", device, supported, frames, buffer_frames)
}

/// Builds and plays an input stream on `device` in `supported`'s format, converting into
/// `frames`. An output device given here is captured in loopback (WASAPI).
///
/// Inputs: The stream's name for logs ("microphone", "computer sound"), the device, its format,
/// the channel for converted blocks, and the frames per delivered block (`None` leaves the host's
/// default).
/// Outputs: The playing stream, or a privacy-safe reason.
/// Side effects: Starts capture. Stream errors are logged with the stream's name: the first one,
/// then at most one a minute with the count held back (`CaptureErrorThrottle`).
pub(crate) fn start_capture(
    stream_name: &'static str,
    device: &cpal::Device,
    supported: cpal::SupportedStreamConfig,
    frames: Sender<Vec<i16>>,
    buffer_frames: Option<u32>,
) -> Result<cpal::Stream, String> {
    let sample_format = supported.sample_format();
    let mut resampler = PcmResampler::new(supported.sample_rate(), supported.channels());
    let mut config: cpal::StreamConfig = supported.into();
    if let Some(frame_count) = buffer_frames {
        config.buffer_size = cpal::BufferSize::Fixed(frame_count);
    }
    let mut throttle = CaptureErrorThrottle::default();
    let on_error = move |error: cpal::Error| {
        if let Some(held_back) = throttle.note(std::time::Instant::now()) {
            tracing::warn!(
                stream = stream_name,
                error = %error,
                errors_held_back = held_back,
                "A capture stream reported an error"
            );
        }
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
        other => return Err(format!("unsupported capture sample format {other}")),
    }
    .map_err(|error| format!("unable to open the capture stream: {error}"))?;
    stream
        .play()
        .map_err(|error| format!("unable to start the capture stream: {error}"))?;
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
