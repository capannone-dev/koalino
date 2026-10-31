#!/bin/bash
# Installs what the GPU builds need into $NOTE_TAKER_TOOLCHAINS/vulkan-sdk (default ~/toolchains):
# the Vulkan headers, pinned by hash, the build machine's glslc shader compiler, and an empty link
# library. The app links no Vulkan code at build time; see core/adapters/ai/vulkan_loader_shim.rs.
set -euo pipefail
T=${NOTE_TAKER_TOOLCHAINS:-$HOME/toolchains}
VULKAN_HEADERS_VERSION=1.4.309
VULKAN_HEADERS_SHA256=437925ada160d86ed763d29dcb9318c1bb0d024d7deaf77bc7c170b8eb6b6f10
if ! command -v glslc >/dev/null; then
  echo "glslc is missing: install your distribution's shaderc package" >&2
  exit 1
fi
mkdir -p "$T/vulkan-sdk/bin" "$T/vulkan-sdk/lib"
curl -sSfL -o "$T/vulkan-headers.tar.gz" \
  "https://github.com/KhronosGroup/Vulkan-Headers/archive/refs/tags/v$VULKAN_HEADERS_VERSION.tar.gz"
echo "$VULKAN_HEADERS_SHA256  $T/vulkan-headers.tar.gz" | sha256sum -c -
rm -rf "$T/vulkan-sdk/include"
tar -xzf "$T/vulkan-headers.tar.gz" -C "$T/vulkan-sdk" --strip-components=1 \
  "Vulkan-Headers-$VULKAN_HEADERS_VERSION/include"
rm "$T/vulkan-headers.tar.gz"
ln -sf "$(command -v glslc)" "$T/vulkan-sdk/bin/glslc"
ar rcs "$T/vulkan-sdk/lib/libvulkan.a"
# zig's linker wants a shared library for the same name on Linux targets.
cc -shared -x c /dev/null -o "$T/vulkan-sdk/lib/libvulkan.so"
echo "Vulkan headers $VULKAN_HEADERS_VERSION"
