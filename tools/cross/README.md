<!-- How to cross-build the daemon and CLI, with whisper.cpp, for Windows and Android from Linux. -->

# Cross builds (Windows and Android)

The Rust core builds on Linux with the system toolchain. These scripts add what the Windows and
Android builds of `daemon` and `cli` need, including whisper.cpp for on-device transcription
(`--features local-ai`).

| File | Purpose |
| --- | --- |
| `fetch.sh` | Installs into `$NOTE_TAKER_TOOLCHAINS` (default `~/toolchains`): rustup with Rust 1.99.1 and the `x86_64-pc-windows-gnu` and `aarch64-linux-android` targets, zig 0.14.1 and cargo-zigbuild, Android NDK r27c, cmake (in a Python venv), the Whisper `ggml-base.bin` model, checked against its published SHA-1, Microsoft's ONNX Runtime for Windows, checked against a recorded SHA-256, and the diarization models, exported with `tools/diarization/export_models.py` in a Python venv |
| `fetch-flutter.sh` | Installs the Flutter SDK 3.47.8, a Temurin JDK 21, and the Android SDK (platform 36, build-tools 36.0.0, NDK 28.2) with its licences accepted, plus ninja for the Linux desktop build; run after `fetch.sh` |
| `build-flutter-native.sh` | Builds the core library with whisper.cpp and stages it for the Flutter app: `linux` into `ui/native/linux/`, `android` into the APK's `jniLibs/arm64-v8a/` with `libc++_shared.so`, `windows` (release, cargo-zigbuild) into `ui/native/windows/note_taker_core.dll`. It also stages the diarization models and licences (`ui/native/models/diarization/`, or `ui/native/android-assets/models/diarization/` on Android), and on Windows `onnxruntime.dll` (`ui/native/windows/onnxruntime/`). Desktop builds also stage the read-only MCP server `note_taker_mcp` (#112), built without AI features, into `ui/native/linux/` or `ui/native/windows/` (installed beside the app's executable). It also stages the fetched `ggml-base.bin` so the app transcribes out of the box: `ui/native/models/` (installed into the Linux and Windows bundles' `data/models`) and `ui/native/android-assets/models/` (an uncompressed APK asset, about 148 MB of the APK) |
| `fetch-onnxruntime-android.sh` | Downloads Microsoft's ONNX Runtime 1.29.0 for Android with the QNN provider (MIT), checks it against a recorded SHA-256, and keeps only `libonnxruntime.so` in `onnxruntime-android/`. `build-flutter-native.sh android` copies it into the APK, for diarization and Whisper on the NPU (#75). Qualcomm's QNN libraries are not fetched: the app downloads them on the owner's request |
| `fetch-debian-sysroot.sh` | Installs a Debian 13 sysroot (hash-pinned packages) into `debian-13-sysroot`, for every Linux build that leaves this machine. Needs `bsdtar`, `clang` and `lld` |
| `debian-13.sh` | Source after `env.sh`: points a build of `x86_64-unknown-linux-gnu` at that sysroot, in its own `target-debian` directory, and defines `check_debian_13_compatible` |
| `package-linux-server.sh` | Builds `daemon`, `cli` and `browser-host` (the browser extension's host) with every feature, checks them, and assembles `dist/note-taker-server`: launchers, programs and models, ready for `vm-deploy` |
| `env.sh` | Source before a cross build or a Flutter build. Uses the rustup toolchain, a separate target directory (`$NOTE_TAKER_TOOLCHAINS/target`, so the host's `target/` is untouched), and the CMake toolchain files below |
| `fetch-opencl.sh` | Installs the OpenCL headers (hash-pinned) for the Android build's efficient mode. Nothing links OpenCL at build time. |
| `fetch-vulkan.sh` | Installs the Vulkan headers (hash-pinned), a link to the system's `glslc` and an empty link library into `vulkan-sdk`, for the Linux and Windows GPU builds. Needs the system's shaderc package |
| `cmake/` | CMake toolchain files for whisper.cpp; see its README |

```sh
tools/cross/fetch.sh                      # once
tools/cross/fetch-flutter.sh              # once, for the Flutter app
tools/cross/fetch-vulkan.sh               # once, for the Linux and Windows app builds
tools/cross/fetch-opencl.sh               # once, for the Android app build
tools/cross/fetch-debian-sysroot.sh       # once, for the Linux app and server builds
source tools/cross/env.sh
cargo zigbuild --release --features local-ai --target x86_64-pc-windows-gnu --bin daemon --bin cli
cargo build    --release --features local-ai --target aarch64-linux-android --bin daemon --bin cli
```

Outputs are under `$CARGO_TARGET_DIR/<target>/release/`.

## Linux builds that run elsewhere

A program built here against this machine's glibc refuses to start on an older system. The linux
case of `build-flutter-native.sh` and `package-linux-server.sh` therefore build against the Debian 13
sysroot, and fail if a file still needs more than Debian 13's glibc 2.41 or `GLIBCXX_3.4.33`. Zig is
not used for this target because the prebuilt ONNX Runtime needs libstdc++, not zig's libc++.

```sh
tools/cross/package-linux-server.sh
vm-deploy --vm deb-shell dist/note-taker-server
ssh test-deb-shell 'NOTE_TAKER_DATABASE_PATH=~/data/notes.sqlite3 ~/app/cli meetings list'

tools/cross/build-flutter-native.sh linux
(source tools/cross/env.sh; cd ui && flutter build linux --release)
vm-deploy --vm deb-gui ui/build/linux/x64/release/bundle
```

The target machine needs `libstdc++6`, and the desktop app also `libasound2t64`. Both programs find a
GPU at run time, and use the CPU when there is no Vulkan loader.

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
