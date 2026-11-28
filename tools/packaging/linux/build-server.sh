#!/usr/bin/env bash
# Builds the Linux server natively and packs its release tarball (#194, #195), on Ubuntu 24.04
# (GitHub's ubuntu-24.04 and ubuntu-24.04-arm runners, see .github/workflows/server-build.yml).
# Ubuntu 24.04's glibc 2.39 keeps the programs running on Debian 13 and Ubuntu 24.04 or newer; the
# script fails if one needs more than Debian 13 has.
#
# CPU level, fixed rather than the build machine's (-march=native):
#   x86_64  AVX2, FMA and F16C, as tools/cross/package-linux-server.sh builds it;
#   arm64   baseline ARMv8-A, so the server also runs on a Raspberry Pi 4 (Cortex-A72).
#
# Usage: build-server.sh VERSION
# Needs: Rust, clang, cmake, pkg-config, ALSA, Vulkan headers and glslc, OpenSSL, Node.js 20+ and
# npm; the models in $NOTE_TAKER_MODELS (ggml-base.bin and diarization/*.onnx, see
# tools/arm64/fetch-models.sh).
# Output: dist/koalino-server-linux-ARCH-VERSION.tar.gz (ARCH x86_64 or arm64), one folder laid
# out by server/assemble.sh.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
repo=$(realpath "$here/../../..")
VERSION=${1:?usage: build-server.sh VERSION}
MODELS=${NOTE_TAKER_MODELS:?set NOTE_TAKER_MODELS to the folder holding the models}
cd "$repo"

case $(uname -m) in
  x86_64)
    arch=x86_64
    export GGML_NATIVE=OFF GGML_AVX=ON GGML_AVX2=ON GGML_FMA=ON GGML_F16C=ON ;;
  aarch64)
    arch=arm64
    export GGML_NATIVE=OFF ;;
  *) echo "unsupported machine $(uname -m)" >&2; exit 1 ;;
esac

# Fails when a built file needs a newer C or C++ library than Debian 13 has (glibc 2.41,
# GLIBCXX_3.4.33), as tools/cross/debian-13.sh checks the cross build.
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

cargo build --release --features local-ai,microphone,vad,diarization,whisper-rs/vulkan \
  --bin daemon --bin cli --bin browser-host
built=target/release
check_debian_13_compatible "$built/daemon" "$built/cli" "$built/browser-host"

name=koalino-server-linux-$arch-$VERSION
mkdir -p dist
"$here/server/assemble.sh" "$built" "$MODELS" "dist/$name" >/dev/null
tar -czf "dist/$name.tar.gz" -C dist "$name"
rm -rf "dist/$name"
echo "dist/$name.tar.gz"
