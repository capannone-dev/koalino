#!/usr/bin/env bash
# Builds the Rust core for the F-Droid build of Koalino (#163), from source only, into the APK's
# jniLibs. Unlike tools/cross/build-flutter-native.sh it uses only the environment it is given
# (F-Droid's NDK, the Rust toolchain on PATH), and leaves out everything F-Droid cannot build:
#   - speaker separation and the NPU (`diarization`, `npu-whisper`), which need Microsoft's
#     prebuilt ONNX Runtime and, for the NPU, Qualcomm's proprietary libraries;
#   - the bundled models: the app downloads Whisper from whisper.cpp's source on request.
# Whisper (whisper.cpp, built here), efficient mode (OpenCL headers, built against; the phone's
# own OpenCL library is opened at run time), speech-pause chunking and the microphone stay.
#
# Usage: ANDROID_NDK=<ndk r27c> tools/fdroid/build-core.sh [arm64|x86_64]   (default arm64;
#        x86_64 is for testing the same build on the Android emulator)
# Needs: cargo with the Android target for the ABI, cmake, clang (bindgen), curl.
# Side effects: writes ui/android/app/src/main/jniLibs/<abi>/ (core and libc++_shared.so) and
# downloads the OpenCL headers (pinned by SHA-256) into $CARGO_TARGET_DIR or ./target.
set -euo pipefail
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$repo"
: "${ANDROID_NDK:?set ANDROID_NDK to the Android NDK r27c}"
abi=${1:-arm64}

ndk_bin=$ANDROID_NDK/toolchains/llvm/prebuilt/linux-x86_64/bin
case "$abi" in
  arm64)
    triple=aarch64-linux-android
    jni_abi=arm64-v8a
    features=local-ai,mobile-gpu,microphone,vad
    ;;
  x86_64)
    triple=x86_64-linux-android
    jni_abi=x86_64
    features=local-ai,microphone,vad # no OpenCL efficient mode on the emulator
    ;;
  *)
    echo "usage: $0 [arm64|x86_64]" >&2
    exit 2
    ;;
esac
target_dir=${CARGO_TARGET_DIR:-$repo/target}
env_triple=${triple//-/_}
upper_triple=${env_triple^^}

# The OpenCL headers that whisper.cpp's efficient mode compiles against (no library is linked).
opencl_version=2024.10.24
opencl_sha256=159f2a550592bae49859fee83d372acd152328fdf95c0dcd8b9409f8fad5db93
opencl_dir=$target_dir/opencl-headers-$opencl_version
if [ ! -d "$opencl_dir/CL" ]; then
  mkdir -p "$opencl_dir"
  curl -sSfL -o "$opencl_dir.tar.gz" \
    "https://github.com/KhronosGroup/OpenCL-Headers/archive/refs/tags/v$opencl_version.tar.gz"
  echo "$opencl_sha256  $opencl_dir.tar.gz" | sha256sum -c -
  tar -xzf "$opencl_dir.tar.gz" -C "$opencl_dir" --strip-components=1 \
    "OpenCL-Headers-$opencl_version/CL"
  rm "$opencl_dir.tar.gz"
fi

export ANDROID_NDK_HOME=$ANDROID_NDK OPENCL_HEADERS_DIR=$opencl_dir
export "CC_$env_triple=$ndk_bin/${triple}29-clang"
export "AR_$env_triple=$ndk_bin/llvm-ar"
export "CARGO_TARGET_${upper_triple}_LINKER=$ndk_bin/${triple}29-clang"
export "CMAKE_TOOLCHAIN_FILE_$env_triple=$repo/tools/cross/cmake/android-$abi.cmake"
export "BINDGEN_EXTRA_CLANG_ARGS_$env_triple=--sysroot=$ndk_bin/../sysroot"

cargo build --release --locked --features "$features" --lib --target "$triple"

jni=ui/android/app/src/main/jniLibs/$jni_abi
rm -rf "$jni"
mkdir -p "$jni"
cp "$target_dir/$triple/release/libnote_taker_core.so" "$jni/"
cp "$ndk_bin/../sysroot/usr/lib/$triple/libc++_shared.so" "$jni/"
echo "core for $jni_abi in $jni"
