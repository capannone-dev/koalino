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

# The folder is assembled as in CI (#194), from these programs and the fetched models.
"$REPO/tools/packaging/linux/server/assemble.sh" "$built" "$T/models" "$DIST" >/dev/null
echo "packaged $DIST"
