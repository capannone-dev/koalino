#!/bin/bash
# Builds the daemon, the CLI and the browser extension's host (browser-host) with every feature against the Debian 13 sysroot, and assembles them with
# their models into dist/note-taker-server, ready to copy to a Debian 13 or newer machine:
#   daemon, cli    launchers that point the programs at the models beside them
#   bin/           the programs, browser-host included: run `bin/browser-host install` once (they need libstdc++6, present on Debian 13 by default)
#   models/        the Whisper model, the diarization models and their licences
#   dashboard, web/  the web dashboard as a Next.js standalone server (needs Node.js 20 or newer)
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
  --bin daemon --bin cli --bin browser-host --target "$DEBIAN_TARGET"
built=$CARGO_TARGET_DIR/$DEBIAN_TARGET/release
check_debian_13_compatible "$built/daemon" "$built/cli" "$built/browser-host"

for model in ggml-base.bin diarization/segmentation.onnx diarization/embedding.onnx; do
  if [ ! -f "$T/models/$model" ]; then
    echo "missing $T/models/$model: run tools/cross/fetch.sh" >&2
    exit 1
  fi
done
rm -rf "$DIST"
mkdir -p "$DIST/bin" "$DIST/models/diarization/licenses"
cp "$built/daemon" "$built/cli" "$built/browser-host" "$DIST/bin/"
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
# The web dashboard (#125). `next build` rewrites web/tsconfig.json and web/next-env.d.ts, so
# they are put back afterwards.
saved=$(mktemp -d)
cp web/tsconfig.json web/next-env.d.ts "$saved/"
(cd web && npm install --no-audit --no-fund >/dev/null && KOALINO_WEB_STANDALONE=1 npx next build >/dev/null)
cp "$saved/tsconfig.json" "$saved/next-env.d.ts" web/
rm -r "$saved"
mkdir -p "$DIST/web/.next"
cp -r web/.next/standalone/. "$DIST/web/"
cp -r web/.next/static "$DIST/web/.next/static"
if [ -d web/public ]; then cp -r web/public "$DIST/web/public"; fi
cat >"$DIST/dashboard" <<'LAUNCHER'
#!/bin/sh
# Serves the web dashboard: open http://localhost:3000. The browser calls the daemon at
# http://127.0.0.1:8080, which accepts only the http://localhost:3000 origin, so keep that address
# and port; from another computer, use an SSH tunnel that forwards both ports.
here=$(dirname "$(readlink -f "$0")")
PORT=3000 HOSTNAME=127.0.0.1 exec node "$here/web/server.js"
LAUNCHER
chmod +x "$DIST/dashboard"
echo "packaged $DIST"
