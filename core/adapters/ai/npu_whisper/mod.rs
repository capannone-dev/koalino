//! Whisper on a Qualcomm NPU through ONNX Runtime's QNN provider (#75).
//!
//! Qualcomm's precompiled `whisper_small_quantized` (w8a16) runs entirely on the Hexagon NPU of
//! Snapdragon 8 Gen 1, 8 Gen 2, 8 Gen 3 and 8 Elite phones (`install::NPU_BUILDS`, #183). The portable parts here are tested on every platform: the log-mel
//! input (`log_mel`), the vocabulary and mel filters read from the shipped whisper.cpp model
//! (`ggml_vocab`), and the greedy decoder loop (`greedy_decoder`). The ONNX Runtime engine
//! (`qnn_engine`) needs the `npu-whisper` feature; the QNN libraries and the model are
//! downloaded on the owner's request, never shipped.

/// Reads Whisper's mel filters and vocabulary from a whisper.cpp model file.
pub mod ggml_vocab;
/// Runs Whisper's greedy decoding over any NPU that can encode and step.
pub mod greedy_decoder;
/// Where the downloaded files live and whether they are complete.
pub mod install;
/// Computes Whisper's log-mel spectrogram and quantizes it for the encoder.
pub mod log_mel;
#[cfg(test)]
mod npu_whisper_tests;
/// The engine on ONNX Runtime's QNN provider.
#[cfg(feature = "npu-whisper")]
pub mod qnn_engine;
/// Transcribes a recording window by window, with timing and a prompt.
pub mod transcriber;
