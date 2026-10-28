#!/bin/bash
# Builds the Rust core's shared library with whisper.cpp and microphone capture
# (`--features local-ai,microphone`) and stages it where the Flutter app's builds pick it up:
#   linux   -> ui/native/linux/libnote_taker_core.so (installed into the bundle's lib/ by CMake)
#   android -> ui/android/app/src/main/jniLibs/arm64-v8a/ (with the NDK's libc++_shared.so)
#   windows -> ui/native/windows/note_taker_core.dll (installed beside the runner by CMake;
#              release only: a debug DLL exports more than Windows' 65,535-symbol limit)
# The staging directories are gitignored. Usage: build-flutter-native.sh linux|android|windows ...
set -euo pipefail
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=env.sh
source "$REPO/tools/cross/env.sh"
cd "$REPO"

for platform in "$@"; do
  case $platform in
    linux)
      cargo build --release --features local-ai,microphone --lib
      mkdir -p ui/native/linux
      cp "$CARGO_TARGET_DIR/release/libnote_taker_core.so" ui/native/linux/
      ;;
    android)
      cargo build --release --features local-ai,microphone --lib --target aarch64-linux-android
      jni=ui/android/app/src/main/jniLibs/arm64-v8a
      mkdir -p "$jni"
      cp "$CARGO_TARGET_DIR/aarch64-linux-android/release/libnote_taker_core.so" "$jni/"
      cp "$NDK_BIN/../sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" "$jni/"
      ;;
    windows)
      cargo zigbuild --release --features local-ai,microphone --lib --target x86_64-pc-windows-gnu
      mkdir -p ui/native/windows
      cp "$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/note_taker_core.dll" ui/native/windows/
      ;;
    *)
      echo "usage: $0 linux|android|windows ..." >&2
      exit 2
      ;;
  esac
  echo "staged $platform"
done
