#!/usr/bin/env bash
# Builds Koalino for 64-bit ARM Linux (#175) and packs the release tarball, on an ARM64 Linux
# machine (GitHub's ubuntu-24.04-arm runner, see .github/workflows/arm64-build.yml).
#
# The build is native rather than cross-compiled from x86_64: Flutter's Linux runner links GTK,
# GStreamer and their dependencies, which a cross build would need as a full ARM64 sysroot.
# Building on Ubuntu 24.04 (glibc 2.39) keeps the result running on Debian 13 and Ubuntu 24.04 or
# newer, as the x86_64 build does.
#
# The core is built as on x86_64 (whisper.cpp with Vulkan, microphone and computer sound,
# speech-pause chunking, diarization with ONNX Runtime linked in), but for the baseline ARMv8-A
# CPU, so that it also runs on a Raspberry Pi 4 (Cortex-A72, no dot-product instructions).
#
# Usage: tools/arm64/build-linux-arm64.sh VERSION
# Needs: Rust, Flutter, clang, cmake, ninja, pkg-config, GTK 3, GStreamer, ALSA, Vulkan headers
# and glslc, OpenSSL and a JDK (the workflow installs them); the models in $NOTE_TAKER_MODELS
# (ggml-base.bin and diarization/{segmentation,embedding}.onnx).
# Output: dist/koalino-linux-arm64-VERSION.tar.gz, one folder holding the app bundle, laid out
# like koalino-linux-x86_64-VERSION.tar.gz so tools/packaging/linux/ repackages it the same way.
set -euo pipefail
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
VERSION=${1:?usage: build-linux-arm64.sh VERSION}
MODELS=${NOTE_TAKER_MODELS:?set NOTE_TAKER_MODELS to the folder holding the models}
cd "$REPO"

if [ "$(uname -m)" != aarch64 ]; then
  echo "build-linux-arm64.sh builds natively: run it on an ARM64 Linux machine" >&2
  exit 1
fi
for file in ggml-base.bin diarization/segmentation.onnx diarization/embedding.onnx; do
  [ -f "$MODELS/$file" ] || { echo "missing $MODELS/$file" >&2; exit 1; }
done

# Fails when a built file needs a newer C or C++ library than Debian 13 has (glibc 2.41,
# GLIBCXX_3.4.33), as tools/cross/debian-13.sh checks the x86_64 build.
# Inputs: The files to check. Side effects: Prints the offending file.
check_debian_13_compatible() {
  local file limit newest
  for file in "$@"; do
    for limit in GLIBC_2.41 GLIBCXX_3.4.33; do
      newest=$(objdump -T "$file" | grep -o "${limit%_*}_[0-9.]*" | sort -Vu | tail -1 || true)
      if [ -n "$newest" ] && [ "$(printf '%s\n%s\n' "$newest" "$limit" | sort -V | tail -1)" != "$limit" ]; then
        echo "$file needs $newest, newer than Debian 13's $limit" >&2
        return 1
      fi
    done
  done
}

# The baseline ARMv8-A instruction set rather than the build machine's own (-mcpu=native).
export GGML_NATIVE=OFF
cargo build --release --features local-ai,microphone,computer-sound,vad,diarization,whisper-rs/vulkan --lib
cargo build --release --bin note_taker_mcp --bin browser-host
RELEASE=target/release
check_debian_13_compatible "$RELEASE/libnote_taker_core.so" "$RELEASE/note_taker_mcp" "$RELEASE/browser-host"

# The same staging folders tools/cross/build-flutter-native.sh fills; ui/linux/CMakeLists.txt
# installs them into the bundle.
mkdir -p ui/native/linux ui/native/models/diarization/licenses
cp "$RELEASE/libnote_taker_core.so" "$RELEASE/note_taker_mcp" "$RELEASE/browser-host" ui/native/linux/
cp "$MODELS/ggml-base.bin" ui/native/models/
cp "$MODELS/diarization/segmentation.onnx" "$MODELS/diarization/embedding.onnx" ui/native/models/diarization/
cp tools/diarization/licenses/* ui/native/models/diarization/licenses/

(cd ui && flutter build linux --release)
BUNDLE=ui/build/linux/arm64/release/bundle
[ -x "$BUNDLE/note_taker_ui" ] || { echo "no app in $BUNDLE" >&2; exit 1; }
check_debian_13_compatible "$BUNDLE/note_taker_ui" "$BUNDLE"/lib/*.so

NAME=koalino-linux-arm64-$VERSION
mkdir -p dist
rm -rf "dist/$NAME"
cp -a "$BUNDLE" "dist/$NAME"
tar -czf "dist/$NAME.tar.gz" -C dist "$NAME"
rm -rf "dist/$NAME"
echo "dist/$NAME.tar.gz"
