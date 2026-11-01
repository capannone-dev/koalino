#!/bin/bash
# Builds the daemon and CLI with every feature against the Debian 13 sysroot, and assembles them with
# their models into dist/note-taker-server, ready to copy to a Debian 13 or newer machine:
#   daemon, cli    launchers that point the programs at the models beside them
#   bin/           the programs (they need libstdc++6, present on Debian 13 by default)
#   models/        the Whisper model, the diarization models and their licences
# Settings already in the environment win over the launchers' defaults. The folder is gitignored.
set -euo pipefail
REPO=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
# shellcheck source=env.sh
source "$REPO/tools/cross/env.sh"
# shellcheck source=debian-13.sh
source "$CROSS_DIR/debian-13.sh"
cd "$REPO"
DIST=dist/note-taker-server

# A fixed CPU level, because -march=native would tie the programs to the build machine's CPU.
export GGML_NATIVE=OFF GGML_AVX=ON GGML_AVX2=ON GGML_FMA=ON GGML_F16C=ON
cargo build --release --features local-ai,microphone,vad,diarization,whisper-rs/vulkan \
  --bin daemon --bin cli --target "$DEBIAN_TARGET"
built=$CARGO_TARGET_DIR/$DEBIAN_TARGET/release
check_debian_13_compatible "$built/daemon" "$built/cli"

for model in ggml-base.bin diarization/segmentation.onnx diarization/embedding.onnx; do
  if [ ! -f "$T/models/$model" ]; then
    echo "missing $T/models/$model: run tools/cross/fetch.sh" >&2
    exit 1
  fi
done
rm -rf "$DIST"
mkdir -p "$DIST/bin" "$DIST/models/diarization/licenses"
cp "$built/daemon" "$built/cli" "$DIST/bin/"
cp "$T/models/ggml-base.bin" "$DIST/models/"
cp "$T"/models/diarization/*.onnx "$DIST/models/diarization/"
cp tools/diarization/licenses/* "$DIST/models/diarization/licenses/"
for program in daemon cli; do
  cat >"$DIST/$program" <<LAUNCHER
#!/bin/sh
# Runs $program with the models beside it. Settings already in the environment win.
here=\$(dirname "\$(readlink -f "\$0")")
export NOTE_TAKER_WHISPER_MODEL="\${NOTE_TAKER_WHISPER_MODEL:-\$here/models/ggml-base.bin}"
export NOTE_TAKER_ENABLE_DIARIZATION="\${NOTE_TAKER_ENABLE_DIARIZATION:-true}"
export NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL="\${NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL:-\$here/models/diarization/segmentation.onnx}"
export NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL="\${NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL:-\$here/models/diarization/embedding.onnx}"
exec "\$here/bin/$program" "\$@"
LAUNCHER
  chmod +x "$DIST/$program"
done
echo "packaged $DIST"
