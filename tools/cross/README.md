<!-- How to cross-build the daemon and CLI, with whisper.cpp, for Windows and Android from Linux. -->

# Cross builds (Windows and Android)

The Rust core builds on Linux with the system toolchain. These scripts add what the Windows and
Android builds of `daemon` and `cli` need, including whisper.cpp for on-device transcription
(`--features local-ai`).

| File | Purpose |
| --- | --- |
| `fetch.sh` | Installs into `$NOTE_TAKER_TOOLCHAINS` (default `~/toolchains`): rustup with Rust 1.99.1 and the `x86_64-pc-windows-gnu` and `aarch64-linux-android` targets, zig 0.14.1 and cargo-zigbuild, Android NDK r27c, cmake (in a Python venv), the Whisper `ggml-base.bin` model, checked against its published SHA-1, Microsoft's ONNX Runtime for Windows, checked against a recorded SHA-256, and the diarization models, exported with `tools/diarization/export_models.py` in a Python venv |
| `fetch-flutter.sh` | Installs the Flutter SDK 3.47.8, a Temurin JDK 21, and the Android SDK (platform 36, build-tools 36.0.0, NDK 28.2) with its licences accepted, plus ninja for the Linux desktop build; run after `fetch.sh` |
| `build-flutter-native.sh` | Builds the core library with whisper.cpp and stages it for the Flutter app: `linux` into `ui/native/linux/`, `android` into the APK's `jniLibs/arm64-v8a/` with `libc++_shared.so`, `windows` (release, cargo-zigbuild) into `ui/native/windows/note_taker_core.dll`. It also stages the diarization models and licences (`ui/native/models/diarization/`, or `ui/native/android-assets/models/diarization/` on Android), and on Windows `onnxruntime.dll` (`ui/native/windows/onnxruntime/`). It also stages the fetched `ggml-base.bin` so the app transcribes out of the box: `ui/native/models/` (installed into the Linux and Windows bundles' `data/models`) and `ui/native/android-assets/models/` (an uncompressed APK asset, about 148 MB of the APK) |
| `env.sh` | Source before a cross build or a Flutter build. Uses the rustup toolchain, a separate target directory (`$NOTE_TAKER_TOOLCHAINS/target`, so the host's `target/` is untouched), and the CMake toolchain files below |
| `cmake/` | CMake toolchain files for whisper.cpp; see its README |

```sh
tools/cross/fetch.sh                      # once
tools/cross/fetch-flutter.sh              # once, for the Flutter app
source tools/cross/env.sh
cargo zigbuild --release --features local-ai --target x86_64-pc-windows-gnu --bin daemon --bin cli
cargo build    --release --features local-ai --target aarch64-linux-android --bin daemon --bin cli
```

Outputs are under `$CARGO_TARGET_DIR/<target>/release/`.

Notes:

- **Windows must be built in release.** In a debug build, the core's `cdylib` (the FFI DLL)
  exports more than the 65,535 symbols a Windows DLL can hold, and linking fails.
- **Android needs `libc++_shared.so`** next to the binary, or on `LD_LIBRARY_PATH`. It ships in
  the NDK at `toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/aarch64-linux-android/`.
- **Run with the model.** Set `NOTE_TAKER_WHISPER_MODEL` to the model path when running.
  Multilingual `ggml-base` transcribed whisper.cpp's 11 s `jfk.wav` sample in these times:

  | Device | Time |
  | --- | --- |
  | Galaxy S23 | 9.7 s |
  | win11 VM (1 vCPU) | 13.3 s |
  | Host | 3.6 s |

The binaries were verified on the win11 VM and on an Android 16 phone. Both recorded, transcribed,
and synced with the Debian VMs.
