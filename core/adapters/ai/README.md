<!-- Documents optional local AI infrastructure kept outside the application layer. -->

# Local AI adapters

This directory contains opt-in local AI infrastructure. It is compiled only with
`--features local-ai`; the default crate, `make rust-test`, and application layer remain free of
Whisper/whisper.cpp types and native model initialization.

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
