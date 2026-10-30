//! Local speaker diarization (Issue #44): decodes the recorder's chunk audio, segments it into
//! speaker turns with a segmentation model, and embeds each speaker with an embedding model.
//! The turn logic is model-agnostic and tested with fakes; the ONNX-backed models live behind the
//! `diarization` feature.

/// Runs the segmentation and embedding models with ONNX Runtime.
#[cfg(feature = "diarization")]
pub mod onnx_models;
/// Decodes pyannote segmentation-3.0's powerset frame scores.
pub mod powerset;
/// Decodes the recorder's 16 kHz mono PCM16 WAV chunks.
pub mod wav_input;
/// Assembles speaker turns from windowed segmentation and per-speaker embeddings.
pub mod windowed_engine;

#[cfg(all(test, feature = "diarization"))]
mod onnx_models_tests;
#[cfg(test)]
mod powerset_tests;
#[cfg(test)]
mod wav_input_tests;
#[cfg(test)]
mod windowed_engine_tests;
