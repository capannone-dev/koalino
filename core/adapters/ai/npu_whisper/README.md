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
| `ggml_vocab.rs` | Reads the mel filters and the vocabulary from a multilingual whisper.cpp model's header (the shipped `ggml-base.bin`), so no tokenizer library is needed, and tokenizes prompts as whisper.cpp does (GPT-2's word split, then the longest known token). |
| `greedy_decoder.rs` | The `WhisperNpu` trait (encode a window, restart, take one decoder step), greedy decoding with an optional prompt and Whisper's timestamp rules, token probabilities from the dequantized logits, the split of a window's tokens into timed segments, and the share-out of a segment's time among its tokens. |
| `transcriber.rs` | Runs a recording window by window: the next window starts after the last finished segment, each window is prompted with the earlier text, segments that are only `[BLANK_AUDIO]` and windows the no-speech rule rejects are dropped, and the words get approximate times and confidences. |
| `qnn_engine.rs` | The ONNX Runtime sessions on the NPU and `QnnWhisperEngine`, the `TranscriptionEngine`. |
| `npu_whisper_tests.rs` | Tests of the portable parts on every platform, and ignored on-device tests. |

## How a recording is transcribed

1. The recording's 16 kHz samples are taken 30 s at a time.
2. Each window becomes a log-mel spectrogram with the vocabulary model's filters, quantized with
   the scale and zero point in the model's `metadata.json`.
3. The encoder fills the decoder's cross-attention caches. The decoder then takes one token per
   step: its self-attention caches start at the quantized zero (128) and its right-aligned
   attention mask opens one position per step, up to the model's 200-position window.
4. The prefix is `<|startofprev|>` with up to 64 prompt tokens (the chunk's previous text and the
   glossary, then the text decoded so far), then the start-of-transcript, language and transcribe
   tokens. Without a set language, the model chooses it after the start-of-transcript alone and
   the decoder is restarted before the prompt.
5. Decoding is greedy under Whisper's timestamp rules and stops at end-of-text. The window's
   finished segments are kept; the next window starts at the last finished segment's end.
6. A window is dropped when `<|nospeech|>` was likelier than 60% and its text tokens' mean log
   probability is below −1 (Whisper's rule). This keeps made-up phrases out of silence.
7. Each segment's time is shared out among its tokens by byte length, so words have approximate
   times. Each word's confidence is its least sure token's probability.

Measured on the S23 with 83 s of public FLEURS Italian test clips joined together: 6.2 s to
transcribe (0.075× real time) and a word error rate of 3.4%. Without the no-speech rule it was
8.2%, because of a phrase made up in the trailing silence. The JFK clip (11 s) takes 0.56 s after
a 2.3 s model load. In the spike, on separate FLEURS clips, the error rate was 8.6% against 8.0%
for the same Whisper Small unquantized (faster-whisper, greedy) on a computer.

The glossary prompt reaches the model, but on that clip it did not fix the terms given and moved
the error rate by about two words either way. Its benefit on the NPU is not shown yet.

The after-meeting pass uses the NPU too when the NPU is on and no separate pass model was chosen
(`pass_engine_choice` in `core/bin/pipeline_composition.rs`). It falls back to whisper.cpp with
the live model if the NPU will not open.

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
