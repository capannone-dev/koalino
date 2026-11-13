//! Captures what the computer plays with `cpal`, as a second track beside the microphone (#108,
//! `computer-sound` feature).
//!
//! - Windows: the default output device in WASAPI loopback (`cpal` opens an input stream on an
//!   output device that way). Loopback sends nothing while nothing plays; the recorder places the
//!   samples by the microphone's clock, so the silence costs nothing.
//! - Linux: the monitor source of the default sink, through `cpal`'s PulseAudio host, which also
//!   serves PipeWire through `pipewire-pulse`. PulseAudio names a sink's monitor `<sink>.monitor`.
//! - Elsewhere (Android, macOS): unavailable; Android forbids capturing other apps' audio without
//!   a screen-capture grant, and macOS needs a virtual device.
//!
//! The stream runs on its own thread, as the microphone's does, converted to 16 kHz mono PCM16.
//! On Linux it asks for 50 ms blocks (`monitor_buffer_frames`), not PulseAudio's default 2 s, so
//! the track stays in step with the microphone.
//! Audio is never logged; errors name the failing step only.

use std::sync::mpsc::Sender;

#[cfg(any(target_os = "windows", target_os = "linux"))]
use cpal::traits::{DeviceTrait, HostTrait};

#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::adapters::audio::cpal_microphone::{open_on_thread, start_capture};
use crate::{
    domain::errors::AudioError,
    ports::audio::{MicrophoneSource, MicrophoneStream},
};

/// What the computer plays, from its default output.
#[derive(Clone, Copy, Debug, Default)]
pub struct CpalComputerSound;

impl CpalComputerSound {
    /// Creates the source; nothing is opened until `open`.
    pub fn new() -> Self {
        Self
    }
}

impl MicrophoneSource for CpalComputerSound {
    /// Opens the default output's sound on a new thread.
    ///
    /// Inputs: The channel for converted blocks.
    /// Outputs: The open stream, or `Unavailable` naming the step that failed (no output, no
    /// monitor source, an unreadable format, or a platform without capture).
    /// Side effects: Spawns the `computer-sound` thread and captures until closed.
    fn open(&self, frames: Sender<Vec<i16>>) -> Result<Box<dyn MicrophoneStream>, AudioError> {
        open_platform(frames)
    }
}

#[cfg(target_os = "windows")]
fn open_platform(frames: Sender<Vec<i16>>) -> Result<Box<dyn MicrophoneStream>, AudioError> {
    open_on_thread("computer-sound", move || {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or_else(|| String::from("no sound output is available"))?;
        let supported = device
            .default_output_config()
            .map_err(|error| format!("unable to read the sound output format: {error}"))?;
        start_capture("computer sound", &device, supported, frames, None)
    })
}

#[cfg(target_os = "linux")]
fn open_platform(frames: Sender<Vec<i16>>) -> Result<Box<dyn MicrophoneStream>, AudioError> {
    open_on_thread("computer-sound", move || {
        let host = cpal::host_from_id(cpal::HostId::PulseAudio)
            .map_err(|error| format!("the sound server is not available: {error}"))?;
        let sink_id = host
            .default_output_device()
            .and_then(|sink| sink.id().ok())
            .ok_or_else(|| String::from("no sound output is available"))?
            .to_string();
        let sources: Vec<(String, cpal::Device)> = host
            .devices()
            .map_err(|error| format!("unable to list the sound devices: {error}"))?
            .filter_map(|device| Some((device.id().ok()?.to_string(), device)))
            .collect();
        let ids: Vec<&str> = sources.iter().map(|(id, _)| id.as_str()).collect();
        let monitor_id = monitor_source_for(&sink_id, &ids)
            .ok_or_else(|| String::from("the sound output has no monitor source"))?
            .to_owned();
        let (_, monitor) = sources
            .into_iter()
            .find(|(id, _)| *id == monitor_id)
            .ok_or_else(|| String::from("the monitor source went away"))?;
        let supported = monitor
            .default_input_config()
            .map_err(|error| format!("unable to read the monitor format: {error}"))?;
        let (min_frames, max_frames) = match supported.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => (*min, *max),
            cpal::SupportedBufferSize::Unknown => (1, u32::MAX),
        };
        let buffer_frames = monitor_buffer_frames(supported.sample_rate(), min_frames, max_frames);
        start_capture(
            "computer sound",
            &monitor,
            supported,
            frames,
            Some(buffer_frames),
        )
    })
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn open_platform(_frames: Sender<Vec<i16>>) -> Result<Box<dyn MicrophoneStream>, AudioError> {
    Err(AudioError::Unavailable {
        reason: String::from("this platform cannot record the computer's sound"),
    })
}

/// Frames per block asked of the monitor stream: 50 ms, within the host's supported range.
///
/// Without a size, PulseAudio delivers a record stream in fragments of about 2 s. The recorder
/// places each batch to end at the microphone's time, so the computer's sound landed 2 s late and
/// the echo detector, which looks 0–300 ms apart, never saw the speakers in the microphone.
/// cpal also uses the size as the server's queue limit, so it is not made smaller than 50 ms.
///
/// Inputs: The stream's sample rate and the supported buffer range in frames.
/// Outputs: The frame count. Side effects: None.
pub fn monitor_buffer_frames(sample_rate_hz: u32, min_frames: u32, max_frames: u32) -> u32 {
    (sample_rate_hz / 20).clamp(min_frames, max_frames.max(min_frames))
}

/// Finds the monitor source of a PulseAudio sink among the devices.
///
/// Inputs: The sink's device ID and every device ID, as `cpal` prints them (`pulseaudio:<name>`).
/// Outputs: The ID of `<sink>.monitor`, or `None`. Side effects: None.
pub fn monitor_source_for<'id>(sink_id: &str, device_ids: &[&'id str]) -> Option<&'id str> {
    let wanted = format!("{sink_id}.monitor");
    device_ids.iter().copied().find(|id| *id == wanted)
}
