#!/usr/bin/env bash
# Builds Koalino for Windows on ARM (#176) and packs the release zip, on an ARM64 Windows machine
# (GitHub's windows-11-arm runner, see .github/workflows/arm64-build.yml), in Git Bash with the
# Visual Studio ARM64 developer environment loaded.
#
# The build is native: Flutter builds a Windows app for the machine it runs on, and the x86_64
# release's cross build (cargo-zigbuild, windows-gnu) has no ARM64 ONNX Runtime to link. So the
# core is built for aarch64-pc-windows-msvc, with whisper.cpp compiled by clang-cl (ggml refuses
# MSVC's own compiler on ARM), for the baseline ARMv8 CPU. Diarization loads Microsoft's ARM64
# onnxruntime.dll from beside the app at run time, as on x86_64. No Vulkan: the x86_64 build's
# Vulkan support is not yet tried on ARM GPUs, so transcription runs on the CPU.
#
# Usage: tools/arm64/build-windows-arm64.sh VERSION
# Needs: Rust with the aarch64-pc-windows-msvc target, Flutter, Visual Studio 2022 with the ARM64
# and Clang tools, cmake, ninja, a JDK, LIBCLANG_PATH for bindgen; the models in
# $NOTE_TAKER_MODELS and the ARM64 ONNX Runtime in $NOTE_TAKER_ONNX_RUNTIME (onnxruntime.dll,
# LICENSE, ThirdPartyNotices.txt), both prepared by the workflow.
# Output: dist/koalino-windows-arm64-VERSION.zip, one folder holding the app, laid out like
# koalino-windows-x86_64-VERSION.zip so tools/packaging/windows/build-installer.sh packs it too.
set -euo pipefail
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
VERSION=${1:?usage: build-windows-arm64.sh VERSION}
MODELS=${NOTE_TAKER_MODELS:?set NOTE_TAKER_MODELS to the folder holding the models}
ONNX_RUNTIME=${NOTE_TAKER_ONNX_RUNTIME:?set NOTE_TAKER_ONNX_RUNTIME to the ARM64 ONNX Runtime folder}
TARGET=aarch64-pc-windows-msvc
cd "$REPO"

for file in "$MODELS/ggml-base.bin" "$MODELS/diarization/segmentation.onnx" \
  "$MODELS/diarization/embedding.onnx" "$ONNX_RUNTIME/onnxruntime.dll"; do
  [ -f "$file" ] || { echo "missing $file" >&2; exit 1; }
done

# whisper.cpp through the cmake crate: Ninja with clang-cl, and the baseline ARMv8 CPU rather
# than the build machine's.
export GGML_NATIVE=OFF CMAKE_GENERATOR=Ninja
export CC_aarch64_pc_windows_msvc=clang-cl CXX_aarch64_pc_windows_msvc=clang-cl
cargo build --release --features local-ai,microphone,computer-sound,vad,diarization --lib --target "$TARGET"
cargo build --release --bin note_taker_mcp --bin browser-host --target "$TARGET"
RELEASE=target/$TARGET/release

# The staging folders ui/windows/CMakeLists.txt installs beside the runner.
STAGE=ui/native/windows
mkdir -p "$STAGE/onnxruntime" ui/native/models/diarization/licenses
cp "$RELEASE/note_taker_core.dll" "$RELEASE/note_taker_mcp.exe" "$RELEASE/browser-host.exe" "$STAGE/"
cp "$ONNX_RUNTIME/onnxruntime.dll" "$STAGE/onnxruntime/"
cp "$ONNX_RUNTIME/LICENSE" "$STAGE/onnxruntime/onnxruntime-LICENSE.txt"
cp "$ONNX_RUNTIME/ThirdPartyNotices.txt" "$STAGE/onnxruntime/onnxruntime-ThirdPartyNotices.txt"
cp "$MODELS/ggml-base.bin" ui/native/models/
cp "$MODELS/diarization/segmentation.onnx" "$MODELS/diarization/embedding.onnx" ui/native/models/diarization/
cp tools/diarization/licenses/* ui/native/models/diarization/licenses/

(cd ui && flutter build windows --release)
APP=ui/build/windows/arm64/runner/Release
[ -f "$APP/note_taker_ui.exe" ] || { echo "no ARM64 app in $APP" >&2; exit 1; }
cp LICENSE THIRD_PARTY_NOTICES.md "$APP/"

NAME=koalino-windows-arm64-$VERSION
mkdir -p dist
rm -rf "dist/$NAME" "dist/$NAME.zip"
cp -r "$APP" "dist/$NAME"
(cd dist && 7z a -tzip -bso0 -bsp0 "$NAME.zip" "$NAME")
rm -rf "dist/$NAME"
echo "dist/$NAME.zip"
