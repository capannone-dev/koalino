#!/bin/bash
# Builds the Rust core's shared library with whisper.cpp, microphone capture, speech-pause
# chunking and diarization (`--features local-ai,microphone,vad,diarization`; Issues #51, #60) and stages it where the Flutter
# app's builds pick it up:
#   linux   -> ui/native/linux/libnote_taker_core.so (installed into the bundle's lib/ by CMake)
#   android -> ui/android/app/src/main/jniLibs/arm64-v8a/ (with the NDK's libc++_shared.so and
#              Microsoft's libonnxruntime.so)
#   android-emulator -> ui/android/app/src/main/jniLibs/x86_64/ (the same for the Android emulator on a
#              PC; testing only: no phone GPU or NPU, and build the APK with
#              --target-platform android-x64)
#   windows -> ui/native/windows/note_taker_core.dll (installed beside the runner by CMake;
#              release only: a debug DLL exports more than Windows' 65,535-symbol limit)
# It also stages the Whisper model from fetch.sh, so the app transcribes out of the box:
#   linux, windows -> ui/native/models/ggml-base.bin (installed into the bundle's data/models)
#   android        -> ui/native/android-assets/models/ggml-base.bin (an uncompressed APK asset)
# It stages the diarization models and their licences too (fetch.sh; Issue #60):
#   linux, windows -> ui/native/models/diarization/ (installed into data/models/diarization)
#   android        -> ui/native/android-assets/models/diarization/ (uncompressed APK assets)
# Windows also stages Microsoft's ONNX Runtime, which the core loads at run time:
#   windows -> ui/native/windows/onnxruntime/ (its contents are installed beside the runner)
# Desktop builds also stage the read-only MCP server for the owner's AI assistant (#112):
#   linux   -> ui/native/linux/note_taker_mcp (installed beside the app's executable)
#   windows -> ui/native/windows/note_taker_mcp.exe (installed beside the runner)
# and the browser extension's native host, browser-host, the same way (#125), so an owner who
# installs only the app can run `browser-host install` from its folder.
# The staging directories are gitignored. Usage: build-flutter-native.sh linux|android|android-emulator|windows ...
set -euo pipefail
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=env.sh
source "$REPO/tools/cross/env.sh"
cd "$REPO"

# Rebuilds whisper.cpp when its build flags changed, because cargo doesn't notice that by itself.
# $1 names the platform, $2 the flags or the toolchain file holding them, $3... cargo's target flags.
refresh_whisper_build() {
  local stamp=$CARGO_TARGET_DIR/.whisper-flags-$1 flags=$2
  shift 2
  [ -f "$flags" ] && flags=$(cat "$flags")
  if [ "$(cat "$stamp" 2>/dev/null)" != "$flags" ]; then
    cargo clean --release -p whisper-rs-sys "$@"
    mkdir -p "$CARGO_TARGET_DIR"
    printf '%s' "$flags" >"$stamp"
  fi
}

# Stops unless fetch-vulkan.sh installed the Vulkan headers and shader compiler.
require_vulkan_headers() {
  if [ ! -f "$VULKAN_SDK/include/vulkan/vulkan.hpp" ] || [ ! -x "$VULKAN_SDK/bin/glslc" ]; then
    echo "missing $VULKAN_SDK: run tools/cross/fetch-vulkan.sh" >&2
    exit 1
  fi
}

# Copies the fetched model to $1/ggml-base.bin unless an identical copy is already there.
stage_model() {
  local source=$T/models/ggml-base.bin
  if [ ! -f "$source" ]; then
    echo "missing $source: run tools/cross/fetch.sh" >&2
    exit 1
  fi
  mkdir -p "$1"
  cmp -s "$source" "$1/ggml-base.bin" || cp "$source" "$1/ggml-base.bin"
}

# Copies the fetched diarization models and their licences to $1/diarization.
stage_diarization_models() {
  local file
  mkdir -p "$1/diarization/licenses"
  for file in segmentation.onnx embedding.onnx; do
    if [ ! -f "$T/models/diarization/$file" ]; then
      echo "missing $T/models/diarization/$file: run tools/cross/fetch.sh" >&2
      exit 1
    fi
    cmp -s "$T/models/diarization/$file" "$1/diarization/$file" ||
      cp "$T/models/diarization/$file" "$1/diarization/$file"
  done
  cp tools/diarization/licenses/* "$1/diarization/licenses/"
}

# Copies the fetched ONNX Runtime DLL and its notices to ui/native/windows/onnxruntime.
stage_onnx_runtime() {
  if [ ! -f "$T/onnxruntime-win/onnxruntime.dll" ]; then
    echo "missing $T/onnxruntime-win/onnxruntime.dll: run tools/cross/fetch.sh" >&2
    exit 1
  fi
  mkdir -p ui/native/windows/onnxruntime
  cp "$T"/onnxruntime-win/* ui/native/windows/onnxruntime/
}

for platform in "$@"; do
  case $platform in
    linux)
      # A fixed CPU level, because -march=native would tie the app to the build machine's CPU. Vulkan
      # gives desktop GPUs a large speed-up (#70, #71).
      export GGML_NATIVE=OFF GGML_AVX=ON GGML_AVX2=ON GGML_FMA=ON GGML_F16C=ON
      # shellcheck source=debian-13.sh
      source "$CROSS_DIR/debian-13.sh"
      refresh_whisper_build linux "$GGML_NATIVE $GGML_AVX $GGML_AVX2 $GGML_FMA $GGML_F16C $(cat "$CROSS_DIR/cmake/debian-13.cmake")" \
        --target "$DEBIAN_TARGET"
      cargo build --release --features local-ai,microphone,computer-sound,vad,diarization,whisper-rs/vulkan --lib --target "$DEBIAN_TARGET"
      unset GGML_NATIVE GGML_AVX GGML_AVX2 GGML_FMA GGML_F16C
      check_debian_13_compatible "$CARGO_TARGET_DIR/$DEBIAN_TARGET/release/libnote_taker_core.so"
      # The read-only MCP server for the owner's AI assistant (#112) needs no AI features.
      cargo build --release --bin note_taker_mcp --bin browser-host --target "$DEBIAN_TARGET"
      check_debian_13_compatible "$CARGO_TARGET_DIR/$DEBIAN_TARGET/release/note_taker_mcp" \
        "$CARGO_TARGET_DIR/$DEBIAN_TARGET/release/browser-host"
      mkdir -p ui/native/linux
      cp "$CARGO_TARGET_DIR/$DEBIAN_TARGET/release/libnote_taker_core.so" ui/native/linux/
      cp "$CARGO_TARGET_DIR/$DEBIAN_TARGET/release/note_taker_mcp" ui/native/linux/
      cp "$CARGO_TARGET_DIR/$DEBIAN_TARGET/release/browser-host" ui/native/linux/
      stage_model ui/native/models
      stage_diarization_models ui/native/models
      ;;
    android)
      refresh_whisper_build android "$CROSS_DIR/cmake/android-arm64.cmake" --target aarch64-linux-android
      if [ ! -d "$OPENCL_HEADERS_DIR/CL" ]; then
        echo "missing $OPENCL_HEADERS_DIR: run tools/cross/fetch-opencl.sh" >&2
        exit 1
      fi
      require_vulkan_headers
      # mobile-gpu adds efficient mode; the phone's OpenCL library is opened at run time (#75).
      # whisper-rs/vulkan adds GPUs other than Adreno, off until the owner turns it on (#184). It
      # links Android's own libvulkan.so, which every phone from Android 7 (the minimum) has; the
      # NDK lacks the C++ headers ggml uses, so they come from the pinned Vulkan headers.
      # npu-whisper runs Whisper on the NPU with Microsoft's ONNX Runtime, staged below (#75).
      if [ ! -f "$T/onnxruntime-android/libonnxruntime.so" ]; then
        echo "missing $T/onnxruntime-android: run tools/cross/fetch-onnxruntime-android.sh" >&2
        exit 1
      fi
      CXXFLAGS_aarch64_linux_android="-I$VULKAN_SDK/include" \
        cargo build --release --features local-ai,mobile-gpu,microphone,vad,diarization,npu-whisper,whisper-rs/vulkan \
        --lib --target aarch64-linux-android
      jni=ui/android/app/src/main/jniLibs/arm64-v8a
      mkdir -p "$jni"
      cp "$CARGO_TARGET_DIR/aarch64-linux-android/release/libnote_taker_core.so" "$jni/"
      cp "$NDK_BIN/../sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" "$jni/"
      cp "$T/onnxruntime-android/libonnxruntime.so" "$jni/"
      stage_model ui/native/android-assets/models
      stage_diarization_models ui/native/android-assets/models
      ;;
    android-emulator)
      refresh_whisper_build android-emulator "$CROSS_DIR/cmake/android-x86_64.cmake" --target x86_64-linux-android
      if [ ! -f "$T/onnxruntime-android-x86_64/libonnxruntime.so" ]; then
        echo "missing $T/onnxruntime-android-x86_64: run tools/cross/fetch-onnxruntime-android.sh" >&2
        exit 1
      fi
      # Vulkan as on phones, so the emulator's software GPU can exercise that path (#184).
      require_vulkan_headers
      CXXFLAGS_x86_64_linux_android="-I$VULKAN_SDK/include" \
        cargo build --release --features local-ai,microphone,vad,diarization,whisper-rs/vulkan \
        --lib --target x86_64-linux-android
      jni=ui/android/app/src/main/jniLibs/x86_64
      mkdir -p "$jni"
      cp "$CARGO_TARGET_DIR/x86_64-linux-android/release/libnote_taker_core.so" "$jni/"
      cp "$NDK_BIN/../sysroot/usr/lib/x86_64-linux-android/libc++_shared.so" "$jni/"
      cp "$T/onnxruntime-android-x86_64/libonnxruntime.so" "$jni/"
      stage_model ui/native/android-assets/models
      stage_diarization_models ui/native/android-assets/models
      ;;
    windows)
      refresh_whisper_build windows "$(cat "$CROSS_DIR"/cmake/windows-gnu-zig.cmake "$CROSS_DIR"/cmake/mingw-library-names.cmake)" \
        --target x86_64-pc-windows-gnu
      RUSTFLAGS="-L native=$VULKAN_SDK/lib" \
        cargo zigbuild --release --features local-ai,microphone,computer-sound,vad,diarization,whisper-rs/vulkan --lib --target x86_64-pc-windows-gnu
      cargo zigbuild --release --bin note_taker_mcp --bin browser-host --target x86_64-pc-windows-gnu
      mkdir -p ui/native/windows
      cp "$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/note_taker_core.dll" ui/native/windows/
      cp "$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/note_taker_mcp.exe" ui/native/windows/
      cp "$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/browser-host.exe" ui/native/windows/
      stage_model ui/native/models
      stage_diarization_models ui/native/models
      stage_onnx_runtime
      ;;
    *)
      echo "usage: $0 linux|android|android-emulator|windows ..." >&2
      exit 2
      ;;
  esac
  echo "staged $platform"
done
