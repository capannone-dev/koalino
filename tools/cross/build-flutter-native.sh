#!/bin/bash
# Builds the Rust core's shared library with whisper.cpp (`--features local-ai`) and stages it
# where the Flutter app's builds pick it up:
#   linux   -> ui/native/linux/libnote_taker_core.so (installed into the bundle's lib/ by CMake)
#   android -> ui/android/app/src/main/jniLibs/arm64-v8a/ (with the NDK's libc++_shared.so)
# Both staging directories are gitignored. Usage: build-flutter-native.sh linux|android ...
set -euo pipefail
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=env.sh
source "$REPO/tools/cross/env.sh"
cd "$REPO"

for platform in "$@"; do
  case $platform in
    linux)
      cargo build --release --features local-ai --lib
      mkdir -p ui/native/linux
      cp "$CARGO_TARGET_DIR/release/libnote_taker_core.so" ui/native/linux/
      ;;
    android)
      cargo build --release --features local-ai --lib --target aarch64-linux-android
      jni=ui/android/app/src/main/jniLibs/arm64-v8a
      mkdir -p "$jni"
      cp "$CARGO_TARGET_DIR/aarch64-linux-android/release/libnote_taker_core.so" "$jni/"
      cp "$NDK_BIN/../sysroot/usr/lib/aarch64-linux-android/libc++_shared.so" "$jni/"
      ;;
    *)
      echo "usage: $0 linux|android ..." >&2
      exit 2
      ;;
  esac
  echo "staged $platform"
done
