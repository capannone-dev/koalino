#!/usr/bin/env bash
# Assembles the Linux server folder from built programs and models (#194): the layout of
# koalino-server-linux-ARCH-VERSION.tar.gz, which build-server-deb.sh and build-server-rpm.sh
# repackage. Shared by tools/cross/package-linux-server.sh (cross build on the maintainer's
# machine) and build-server.sh (native build in CI), so both give the same folder.
#
# Usage: assemble.sh <programs-dir> <models-dir> <output-dir>
#   <programs-dir> holds daemon, cli and browser-host; <models-dir> holds ggml-base.bin and
#   diarization/{segmentation,embedding}.onnx.
# Output, in <output-dir> (replaced if present):
#   daemon, cli    launchers that point the programs at the models beside them
#   bin/           the programs (they need libstdc++6, present on Debian 13 by default)
#   models/        the Whisper model, the diarization models and their licences
#   dashboard, web/  the web dashboard as a Next.js standalone server (needs Node.js 20 or newer)
# Needs: Node.js and npm, for the dashboard. Side effects: runs `npm install` and `next build`
# in web/, then restores the two files next build rewrites.
set -euo pipefail
programs=$(realpath "$1")
models=$(realpath "$2")
dist=$3
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)

for model in ggml-base.bin diarization/segmentation.onnx diarization/embedding.onnx; do
  [ -f "$models/$model" ] || { echo "missing $models/$model" >&2; exit 1; }
done
for program in daemon cli browser-host; do
  [ -x "$programs/$program" ] || { echo "missing $programs/$program" >&2; exit 1; }
done
rm -rf "$dist"
mkdir -p "$dist/bin" "$dist/models/diarization/licenses"
cp "$programs/daemon" "$programs/cli" "$programs/browser-host" "$dist/bin/"
cp "$models/ggml-base.bin" "$dist/models/"
cp "$models"/diarization/segmentation.onnx "$models"/diarization/embedding.onnx "$dist/models/diarization/"
cp "$repo"/tools/diarization/licenses/* "$dist/models/diarization/licenses/"
for program in daemon cli; do
  cat >"$dist/$program" <<LAUNCHER
#!/bin/sh
# Runs $program with the models beside it. Settings already in the environment win.
here=\$(dirname "\$(readlink -f "\$0")")
export NOTE_TAKER_WHISPER_MODEL="\${NOTE_TAKER_WHISPER_MODEL:-\$here/models/ggml-base.bin}"
export NOTE_TAKER_ENABLE_DIARIZATION="\${NOTE_TAKER_ENABLE_DIARIZATION:-true}"
export NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL="\${NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL:-\$here/models/diarization/segmentation.onnx}"
export NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL="\${NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL:-\$here/models/diarization/embedding.onnx}"
exec "\$here/bin/$program" "\$@"
LAUNCHER
  chmod +x "$dist/$program"
done
# The web dashboard (#125). `next build` rewrites web/tsconfig.json and web/next-env.d.ts, so
# they are put back afterwards.
saved=$(mktemp -d)
cp "$repo/web/tsconfig.json" "$repo/web/next-env.d.ts" "$saved/"
(cd "$repo/web" && npm install --no-audit --no-fund >/dev/null && KOALINO_WEB_STANDALONE=1 npx next build >/dev/null)
cp "$saved/tsconfig.json" "$saved/next-env.d.ts" "$repo/web/"
rm -r "$saved"
mkdir -p "$dist/web/.next"
cp -r "$repo/web/.next/standalone/." "$dist/web/"
cp -r "$repo/web/.next/static" "$dist/web/.next/static"
if [ -d "$repo/web/public" ]; then cp -r "$repo/web/public" "$dist/web/public"; fi
cat >"$dist/dashboard" <<'LAUNCHER'
#!/bin/sh
# Serves the web dashboard: open http://localhost:3000. The browser calls the daemon at
# http://127.0.0.1:8080, which accepts only the http://localhost:3000 origin, so keep that address
# and port; from another computer, use an SSH tunnel that forwards both ports.
here=$(dirname "$(readlink -f "$0")")
PORT=3000 HOSTNAME=127.0.0.1 exec node "$here/web/server.js"
LAUNCHER
chmod +x "$dist/dashboard"
echo "$dist"
