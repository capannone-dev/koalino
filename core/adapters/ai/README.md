<!-- Documents optional local AI infrastructure kept outside the application layer. -->

# Local AI adapters

`diarization/` holds the local speaker diarization adapter; see its README. Its ONNX Runtime
models are compiled only with `--features diarization`.

The rest of this directory contains opt-in local AI infrastructure. It is compiled only with
`--features local-ai`; the default crate, `make rust-test`, and application layer remain free of
Whisper/whisper.cpp types and native model initialization.

`whisper_word_timing.rs` builds timed words from whisper.cpp tokens (#69), and `whisper_model_download.rs` downloads
a model from Hugging Face, installing it only when its pinned hash matches (#67). Both compile without `local-ai`.

`compute_resources.rs` decides how inference uses the machine: at most 8 threads on fast cores, the minimum CPU
instructions, and which GPU fits a model (#70, #71). `vulkan_loader_shim.rs` lets a Vulkan build start without Vulkan.
Set `NOTE_TAKER_WHISPER_GPU=off` to force the CPU. The Vulkan build is selected with `whisper-rs/vulkan` in the
cross scripts, not a crate feature, so `--all-features` builds don't need its headers.

`local_whisper_cpp.rs` implements `TranscriptionEngine` using `whisper-rs`, the Rust binding for
whisper.cpp. The adapter owns a `RecordingPcmSource` seam that resolves adapter-local, 16 kHz,
mono floating-point PCM for portable `AudioRecording` metadata. It returns only the existing
domain `Transcript` and `TranscriptionError` values.

The local engine always receives an explicit language from `NOTE_TAKER_WHISPER_LANGUAGE`:
`auto` (the default) detects the language of each chunk, and a code such as `it` or `en` pins it.
Leaving the language unset is not an option, because whisper.cpp's built-in default is `en`, which
transcribes Italian speech as English (requirement 4.2).

To exercise a real model locally, set `NOTE_TAKER_WHISPER_MODEL` to an existing GGML/GGUF model
path and run `cargo test --features local-ai`. No model path is read or model context initialized
when the feature is absent or the environment variable is unset.

When supplied, portable `TranscriptionContext.previous_text` becomes whisper.cpp's initial prompt.
The OpenAI adapter maps the same value to its documented multipart `prompt` field. Neither adapter
logs the context text.
