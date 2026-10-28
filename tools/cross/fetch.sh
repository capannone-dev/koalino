#!/bin/bash
# Installs the cross-compile toolchains for the Windows and Android builds of the daemon and CLI,
# plus the Whisper model, into $NOTE_TAKER_TOOLCHAINS (default ~/toolchains). See README.md.
set -euo pipefail
T=${NOTE_TAKER_TOOLCHAINS:-$HOME/toolchains}
RUST_VERSION=1.99.1 ZIG_VERSION=0.14.1 NDK_VERSION=r27c
# Published SHA-1 of ggml-base.bin (whisper.cpp models/README.md).
WHISPER_BASE_SHA1=465707469ff3a37a2b9b8d8f89f2f99de7299dac
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
