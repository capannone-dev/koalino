#!/bin/bash
# Installs the cross-compile toolchains for the Windows and Android builds of the daemon and CLI,
# plus ONNX Runtime for Windows and the Whisper and diarization models, into $NOTE_TAKER_TOOLCHAINS (default ~/toolchains). See README.md.
set -euo pipefail
T=${NOTE_TAKER_TOOLCHAINS:-$HOME/toolchains}
RUST_VERSION=1.99.1 ZIG_VERSION=0.14.1 NDK_VERSION=r27c
# Published SHA-1 of ggml-base.bin (whisper.cpp models/README.md).
WHISPER_BASE_SHA1=465707469ff3a37a2b9b8d8f89f2f99de7299dac
# Microsoft's Windows build, the version `ort` is designed for, and its SHA-256 as downloaded.
ONNX_RUNTIME_VERSION=1.28.0
ONNX_RUNTIME_WIN_SHA256=abef733dacbe2f571547a7150b479b5cb9cc0df22f96c24983a42cadb1b4f8bc
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
mkdir -p "$T/models"
export RUSTUP_HOME=$T/rustup CARGO_HOME=$T/cargo

curl -sSfL https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init -o "$T/rustup-init"
chmod +x "$T/rustup-init"
"$T/rustup-init" -y --no-modify-path --profile minimal --default-toolchain "$RUST_VERSION" \
  --target x86_64-pc-windows-gnu --target aarch64-linux-android >"$T/rustup.log" 2>&1
echo "rust $RUST_VERSION with Windows and Android targets"

curl -sSfL "https://ziglang.org/download/$ZIG_VERSION/zig-x86_64-linux-$ZIG_VERSION.tar.xz" | tar -xJ -C "$T"
"$T/cargo/bin/cargo" install --locked cargo-zigbuild >"$T/zigbuild.log" 2>&1
echo "zig $ZIG_VERSION and cargo-zigbuild"

curl -sSfL "https://dl.google.com/android/repository/android-ndk-$NDK_VERSION-linux.zip" -o "$T/ndk.zip"
(cd "$T" && unzip -q -o ndk.zip && rm ndk.zip)
echo "Android NDK $NDK_VERSION"

python3 -m venv "$T/cmake-venv" && "$T/cmake-venv/bin/pip" install -q cmake
echo "cmake $("$T/cmake-venv/bin/cmake" --version | head -1)"

curl -sSfL -o "$T/models/ggml-base.bin" \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
echo "$WHISPER_BASE_SHA1  $T/models/ggml-base.bin" | sha1sum -c -

curl -sSfL -o "$T/onnxruntime-win.zip" \
  "https://github.com/microsoft/onnxruntime/releases/download/v$ONNX_RUNTIME_VERSION/onnxruntime-win-x64-$ONNX_RUNTIME_VERSION.zip"
echo "$ONNX_RUNTIME_WIN_SHA256  $T/onnxruntime-win.zip" | sha256sum -c -
mkdir -p "$T/onnxruntime-win"
unzip -q -o -j "$T/onnxruntime-win.zip" -d "$T/onnxruntime-win" \
  "onnxruntime-win-x64-$ONNX_RUNTIME_VERSION/lib/onnxruntime.dll" \
  "onnxruntime-win-x64-$ONNX_RUNTIME_VERSION/LICENSE" \
  "onnxruntime-win-x64-$ONNX_RUNTIME_VERSION/ThirdPartyNotices.txt"
mv "$T/onnxruntime-win/LICENSE" "$T/onnxruntime-win/onnxruntime-LICENSE.txt"
mv "$T/onnxruntime-win/ThirdPartyNotices.txt" "$T/onnxruntime-win/onnxruntime-ThirdPartyNotices.txt"
rm "$T/onnxruntime-win.zip"
echo "ONNX Runtime $ONNX_RUNTIME_VERSION for Windows"

python3 -m venv "$T/diarization-venv"
"$T/diarization-venv/bin/pip" install -q --index-url https://download.pytorch.org/whl/cpu torch torchaudio
"$T/diarization-venv/bin/pip" install -q speechbrain pyannote.audio onnx onnxruntime huggingface_hub
"$T/diarization-venv/bin/python" "$REPO/tools/diarization/export_models.py" --output-dir "$T/models/diarization"
echo "diarization models"
