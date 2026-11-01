#!/bin/bash
# Installs the OpenCL headers, pinned by hash, into $NOTE_TAKER_TOOLCHAINS/opencl-headers (default
# ~/toolchains). The Android build's whisper.cpp needs them for efficient mode (#75). The app links
# no OpenCL code at build time; see core/adapters/ai/opencl_loader_shim.rs.
set -euo pipefail
T=${NOTE_TAKER_TOOLCHAINS:-$HOME/toolchains}
OPENCL_HEADERS_VERSION=2024.10.24
OPENCL_HEADERS_SHA256=159f2a550592bae49859fee83d372acd152328fdf95c0dcd8b9409f8fad5db93
curl -sSfL -o "$T/opencl-headers.tar.gz" \
  "https://github.com/KhronosGroup/OpenCL-Headers/archive/refs/tags/v$OPENCL_HEADERS_VERSION.tar.gz"
echo "$OPENCL_HEADERS_SHA256  $T/opencl-headers.tar.gz" | sha256sum -c -
rm -rf "$T/opencl-headers"
mkdir -p "$T/opencl-headers"
tar -xzf "$T/opencl-headers.tar.gz" -C "$T/opencl-headers" --strip-components=1 \
  "OpenCL-Headers-$OPENCL_HEADERS_VERSION/CL"
rm "$T/opencl-headers.tar.gz"
echo "OpenCL headers $OPENCL_HEADERS_VERSION"
