#!/bin/bash
# Builds the Rust core's shared library with whisper.cpp and microphone capture
# (`--features local-ai,microphone`) and stages it where the Flutter app's builds pick it up:
#   linux   -> ui/native/linux/libnote_taker_core.so (installed into the bundle's lib/ by CMake)
#   android -> ui/android/app/src/main/jniLibs/arm64-v8a/ (with the NDK's libc++_shared.so)
#   windows -> ui/native/windows/note_taker_core.dll (installed beside the runner by CMake;
#              release only: a debug DLL exports more than Windows' 65,535-symbol limit)
# It also stages the Whisper model from fetch.sh, so the app transcribes out of the box:
#   linux, windows -> ui/native/models/ggml-base.bin (installed into the bundle's data/models)
#   android        -> ui/native/android-assets/models/ggml-base.bin (an uncompressed APK asset)
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

for platform in "$@"; do
  case $platform in
    linux)
      cargo build --release --features local-ai,microphone --lib
      mkdir -p ui/native/linux
      cp "$CARGO_TARGET_DIR/release/libnote_taker_core.so" ui/native/linux/
      stage_model ui/native/models
      ;;
    android)
      cargo build --release --features local-ai,microphone --lib --target aarch64-linux-android
      jni=ui/android/app/src/main/jniLibs/arm64-v8a
      mkdir -p "$jni"
      cp "$CARGO_TARGET_DIR/aarch64-linux-android/release/libnote_taker_core.so" "$jni/"
      cp "$NDK_BIN/../sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" "$jni/"
      stage_model ui/native/android-assets/models
      ;;
    windows)
      cargo zigbuild --release --features local-ai,microphone --lib --target x86_64-pc-windows-gnu
      mkdir -p ui/native/windows
      cp "$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/note_taker_core.dll" ui/native/windows/
      stage_model ui/native/models
      ;;
    *)
      echo "usage: $0 linux|android|windows ..." >&2
      exit 2
      ;;
  esac
  echo "staged $platform"
done
