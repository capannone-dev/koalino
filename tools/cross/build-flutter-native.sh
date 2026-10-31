#!/bin/bash
# Builds the Rust core's shared library with whisper.cpp, microphone capture, speech-pause
# chunking and diarization (`--features local-ai,microphone,vad,diarization`; Issues #51, #60) and stages it where the Flutter
# app's builds pick it up:
#   linux   -> ui/native/linux/libnote_taker_core.so (installed into the bundle's lib/ by CMake)
#   android -> ui/android/app/src/main/jniLibs/arm64-v8a/ (with the NDK's libc++_shared.so)
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
# The staging directories are gitignored. Usage: build-flutter-native.sh linux|android|windows ...
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
      refresh_whisper_build linux "$GGML_NATIVE $GGML_AVX $GGML_AVX2 $GGML_FMA $GGML_F16C"
      cargo build --release --features local-ai,microphone,vad,diarization,whisper-rs/vulkan --lib
      unset GGML_NATIVE GGML_AVX GGML_AVX2 GGML_FMA GGML_F16C
      mkdir -p ui/native/linux
      cp "$CARGO_TARGET_DIR/release/libnote_taker_core.so" ui/native/linux/
      stage_model ui/native/models
      stage_diarization_models ui/native/models
      ;;
    android)
      refresh_whisper_build android "$CROSS_DIR/cmake/android-arm64.cmake" --target aarch64-linux-android
      cargo build --release --features local-ai,microphone,vad,diarization --lib --target aarch64-linux-android
      jni=ui/android/app/src/main/jniLibs/arm64-v8a
      mkdir -p "$jni"
      cp "$CARGO_TARGET_DIR/aarch64-linux-android/release/libnote_taker_core.so" "$jni/"
      cp "$NDK_BIN/../sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" "$jni/"
      stage_model ui/native/android-assets/models
      stage_diarization_models ui/native/android-assets/models
      ;;
    windows)
      refresh_whisper_build windows "$(cat "$CROSS_DIR"/cmake/windows-gnu-zig.cmake "$CROSS_DIR"/cmake/mingw-library-names.cmake)" \
        --target x86_64-pc-windows-gnu
      RUSTFLAGS="-L native=$VULKAN_SDK/lib" \
        cargo zigbuild --release --features local-ai,microphone,vad,diarization,whisper-rs/vulkan --lib --target x86_64-pc-windows-gnu
      mkdir -p ui/native/windows
      cp "$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/note_taker_core.dll" ui/native/windows/
      stage_model ui/native/models
      stage_diarization_models ui/native/models
      stage_onnx_runtime
      ;;
    *)
      echo "usage: $0 linux|android|windows ..." >&2
      exit 2
      ;;
  esac
  echo "staged $platform"
done
