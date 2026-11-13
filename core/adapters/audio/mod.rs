//! Groups audio-capture adapter implementations and their contract tests.

/// Holds each live chunk's PCM until a transcription engine reads (and deletes) it.
pub mod chunk_audio_store;
/// Captures what the computer plays with cpal, beside the microphone (#108).
#[cfg(feature = "computer-sound")]
pub mod cpal_computer_sound;
#[cfg(all(test, feature = "computer-sound"))]
mod cpal_computer_sound_tests;
/// Captures the default microphone with cpal (Issue #45).
#[cfg(feature = "microphone")]
pub mod cpal_microphone;
/// A hand-driven microphone for tests.
#[cfg(test)]
pub mod fake_microphone;
/// Holds an open microphone stream and the frames it has delivered (Issue #45).
pub mod live_microphone;
/// Implements `AudioCapture` using a deterministic local stream seam pending OS integration.
pub mod local_recorder;
/// Which apps capture the microphone, for noticing calls (#111).
pub mod microphone_users;
#[cfg(test)]
mod microphone_users_tests;
/// Compresses kept speech with Opus before it is sealed or sent (#88).
pub mod opus_speech;
/// Reads 16 kHz mono PCM from WAV files or raw streams for file and piped recordings.
pub mod pcm_input;
/// Converts device audio at any rate and channel count to 16 kHz mono PCM16 (Issue #45).
pub mod pcm_resampler;
/// Keeps a meeting's speech sealed on disk until the after-meeting pass has read it (#87).
pub mod retained_audio_store;
/// Compresses and keeps a recording's speech on a background thread (#88).
pub mod retained_speech_writer;
/// Implements feature-gated 16 kHz VAD chunking over an AudioCapture source.
pub mod vad_chunker;

#[cfg(test)]
mod chunk_audio_store_tests;
#[cfg(test)]
mod live_microphone_tests;
#[cfg(test)]
mod local_recorder_tests;
#[cfg(test)]
mod opus_speech_tests;
#[cfg(test)]
mod pcm_input_tests;
#[cfg(test)]
mod pcm_resampler_tests;
#[cfg(test)]
mod retained_audio_store_tests;
#[cfg(test)]
mod retained_speech_writer_tests;
#[cfg(test)]
mod vad_chunker_tests;
