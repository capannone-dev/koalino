<!-- Documents Whisper on a Qualcomm NPU through ONNX Runtime's QNN provider (#75). -->

# NPU Whisper

Runs Whisper Small on the Hexagon NPU of a Snapdragon 8 Gen 2 phone (SM8550). The model is
Qualcomm AI Hub's precompiled `whisper_small_quantized` (w8a16), loaded with Microsoft's ONNX
Runtime and its QNN execution provider. The CPU fallback is turned off, so an NPU that cannot run
the model is an error and the composition falls back to whisper.cpp.

Nothing Qualcomm makes is shipped. The Android app downloads the QNN runtime and the model when
the owner asks, in Settings → Transcription model → Transcribe on the NPU (`NpuDownload.kt`), and
checks each archive's SHA-256. The files go into `<data directory>/npu`.

| File | Purpose |
| --- | --- |
| `mod.rs` | Declares the modules; `qnn_engine` needs the `npu-whisper` feature. |
| `install.rs` | Where the downloaded files go (`model/`, `qnn/`), whether they are all there, and whether this phone's chip (`ro.soc.model`) runs the model. |
| `log_mel.rs` | Whisper's 80-band log-mel spectrogram of a 30 s window (a recursive FFT; it matches faster-whisper to 1e-5), and its 16-bit quantization for the encoder. |
| `ggml_vocab.rs` | Reads the mel filters and the vocabulary from a multilingual whisper.cpp model's header (the shipped `ggml-base.bin`), so no tokenizer library is needed. |
| `greedy_decoder.rs` | The `WhisperNpu` trait (encode a window, take one decoder step) and greedy decoding over it, limited to text tokens and end-of-text. |
| `qnn_engine.rs` | The ONNX Runtime sessions on the NPU and `QnnWhisperEngine`, the `TranscriptionEngine`. |
| `npu_whisper_tests.rs` | Tests of the portable parts on every platform, and ignored on-device tests. |

## How a recording is transcribed

1. The recording's 16 kHz samples are cut into 30 s windows.
2. Each window becomes a log-mel spectrogram with the vocabulary model's filters, quantized with
   the scale and zero point in the model's `metadata.json`.
3. The encoder fills the decoder's cross-attention caches. The decoder then takes one token per
   step: its self-attention caches start at the quantized zero (128) and its right-aligned
   attention mask opens one position per step, up to the model's 200-position window.
4. Decoding starts with the start-of-transcript, language, transcribe and no-timestamps tokens,
   and stops at end-of-text.

Limits: no word timing (timestamps are off), no glossary prompt, and no previous-text context
between windows. On the S23 the JFK clip (11 s) takes 0.56 s after a 2.3 s model load. On public
FLEURS Italian test clips (plus JFK) the word error rate was 8.6%, against 8.0% for the same
Whisper Small unquantized (faster-whisper, greedy) on a computer: the 8-bit weights cost little
accuracy.

## Android requirements

- `libonnxruntime.so` is in the APK (MIT, `tools/cross/fetch-onnxruntime-android.sh`).
- The manifest declares `libcdsprpc.so` with `<uses-native-library>`. Without it the app cannot
  reach the NPU and device creation fails with `QNN_DEVICE_ERROR_INVALID_CONFIG`.
- `ADSP_LIBRARY_PATH` names `<data directory>/npu/qnn` before the core starts
  (`NpuDownload.prepareEnvironment`), so the NPU can load `libQnnHtpV73Skel.so`.

## Running the on-device test

Build the test binary with `cargo test --release --features local-ai,vad,npu-whisper --lib
--target aarch64-linux-android --no-run` (after `source tools/cross/env.sh`), push it with
`libonnxruntime.so`, `libc++_shared.so`, the model, the QNN libraries and a public WAV to
`/data/local/tmp`, and run `transcribes_a_wav_on_the_npu` with `--ignored`. Its doc comment names
the variables. ONNX Runtime with QNN aborts at process exit (`pthread_mutex_lock called on a
destroyed mutex`) after the test has passed. The app never unloads it, so this does not affect
the app.
