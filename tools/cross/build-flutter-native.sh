#!/bin/bash
# Builds the Rust core's shared library with whisper.cpp, microphone capture, speech-pause
# chunking and, on desktops, diarization (`--features local-ai,microphone,vad,diarization`; Issues #51, #60) and stages it where the Flutter
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
# Android stays without diarization until it has been run on a device: the app already installs
# models from ui/native/android-assets/models/diarization/ when the APK ships them.
# Windows also stages Microsoft's ONNX Runtime, which the core loads at run time:
#   windows -> ui/native/windows/onnxruntime/ (its contents are installed beside the runner)
# The staging directories are gitignored. Usage: build-flutter-native.sh linux|android|windows ...
set -euo pipefail
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=env.sh
source "$REPO/tools/cross/env.sh"
cd "$REPO"

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
      cargo build --release --features local-ai,microphone,vad,diarization --lib
      mkdir -p ui/native/linux
      cp "$CARGO_TARGET_DIR/release/libnote_taker_core.so" ui/native/linux/
      stage_model ui/native/models
      stage_diarization_models ui/native/models
      ;;
    android)
      cargo build --release --features local-ai,microphone,vad --lib --target aarch64-linux-android
      jni=ui/android/app/src/main/jniLibs/arm64-v8a
      mkdir -p "$jni"
      cp "$CARGO_TARGET_DIR/aarch64-linux-android/release/libnote_taker_core.so" "$jni/"
      cp "$NDK_BIN/../sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" "$jni/"
      stage_model ui/native/android-assets/models
      ;;
    windows)
      cargo zigbuild --release --features local-ai,microphone,vad,diarization --lib --target x86_64-pc-windows-gnu
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
