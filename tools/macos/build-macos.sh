#!/bin/bash
# Builds Koalino for macOS and packages it as a zip, on a Mac with Xcode (#151).
#
# The core is built for Intel and Apple silicon and joined into one universal
# libnote_taker_core.dylib (whisper.cpp, microphone, speech-pause chunking; diarization on Apple
# silicon only, see below; no computer-sound capture yet). Flutter builds Koalino.app, and this script then puts into it:
#   Contents/Frameworks/libnote_taker_core.dylib   (opened by ui/lib/infrastructure/core_library.dart)
#   Contents/MacOS/note_taker_mcp, browser-host     (MCP server #112, extension host #125)
#   Contents/Resources/models/                      (ggml-base.bin and diarization/, with licences)
# and signs the bundle ad hoc: without an Apple Developer ID it is not notarized, so the owner
# allows it once in System Settings → Privacy & Security (see README.md).
#
# Usage: tools/macos/build-macos.sh [VERSION]   (default 0.1.0)
# Needs: Xcode; Rust with the aarch64-apple-darwin and x86_64-apple-darwin targets; Flutter;
# cmake on PATH; the models in $NOTE_TAKER_MODELS (default ~/toolchains/models: ggml-base.bin and
# diarization/{segmentation,embedding}.onnx, as tools/cross/fetch.sh makes them).
# Output: dist/koalino-macos-universal-VERSION.zip
set -euo pipefail
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
VERSION=${1:-0.1.0}
MODELS=${NOTE_TAKER_MODELS:-$HOME/toolchains/models}
export PATH=$HOME/.cargo/bin:$HOME/toolchains/flutter/bin:$HOME/toolchains/build-venv/bin:$PATH
# The oldest macOS Flutter's runner supports; the core and whisper.cpp are built for it too.
export MACOSX_DEPLOYMENT_TARGET=12.0
cd "$REPO"

for file in ggml-base.bin diarization/segmentation.onnx diarization/embedding.onnx; do
  if [ ! -f "$MODELS/$file" ]; then
    echo "missing $MODELS/$file: run tools/cross/fetch.sh, or copy the models here" >&2
    exit 1
  fi
done

TARGETS=(x86_64-apple-darwin aarch64-apple-darwin)
for target in "${TARGETS[@]}"; do
  # A fixed CPU level for Intel Macs (2013 and later), since a native build would tie the app
  # to the build machine's CPU; Apple silicon always has what whisper.cpp uses.
  # Diarization runs on ONNX Runtime, whose last Intel Mac build (1.23.2) is older than the 1.27
  # API the core uses, and `ort` has no Intel Mac prebuilt: Intel Macs get no speaker separation
  # (segments stay unattributed); Apple silicon links ONNX Runtime in statically.
  if [ "$target" = x86_64-apple-darwin ]; then
    export GGML_NATIVE=OFF GGML_AVX=ON GGML_AVX2=ON GGML_FMA=ON GGML_F16C=ON
    FEATURES=local-ai,microphone,vad
  else
    export GGML_NATIVE=OFF
    unset GGML_AVX GGML_AVX2 GGML_FMA GGML_F16C
    FEATURES=local-ai,microphone,vad,diarization
  fi
  cargo build --release --features "$FEATURES" --lib --target "$target"
  cargo build --release --bin note_taker_mcp --bin browser-host --target "$target"
done

universal() { # $1: file name under target/<triple>/release; $2: output path
  lipo -create -output "$2" \
    "target/x86_64-apple-darwin/release/$1" "target/aarch64-apple-darwin/release/$1"
}
STAGE=ui/native/macos
rm -rf "$STAGE" && mkdir -p "$STAGE"
universal libnote_taker_core.dylib "$STAGE/libnote_taker_core.dylib"
install_name_tool -id @rpath/libnote_taker_core.dylib "$STAGE/libnote_taker_core.dylib"
universal note_taker_mcp "$STAGE/note_taker_mcp"
universal browser-host "$STAGE/browser-host"

(cd ui && flutter build macos --release)
APP=ui/build/macos/Build/Products/Release/Koalino.app
mkdir -p "$APP/Contents/Frameworks" "$APP/Contents/Resources/models/diarization/licenses"
cp "$STAGE/libnote_taker_core.dylib" "$APP/Contents/Frameworks/"
cp "$STAGE/note_taker_mcp" "$STAGE/browser-host" "$APP/Contents/MacOS/"
cp "$MODELS/ggml-base.bin" "$APP/Contents/Resources/models/"
cp "$MODELS/diarization/segmentation.onnx" "$MODELS/diarization/embedding.onnx" \
  "$APP/Contents/Resources/models/diarization/"
cp tools/diarization/licenses/* "$APP/Contents/Resources/models/diarization/licenses/"
cp LICENSE THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/"

# Ad hoc: every Mac needs code signed, and Apple silicon refuses unsigned code outright.
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

mkdir -p dist
OUT=dist/koalino-macos-universal-$VERSION.zip
rm -f "$OUT"
ditto -c -k --keepParent "$APP" "$OUT"
echo "macOS package: $REPO/$OUT"
