#!/usr/bin/env bash
# Builds a single-file Flatpak bundle of the Koalino desktop app from the Linux release tarball.
#
# Usage: build-flatpak.sh <koalino-linux-ARCH-VERSION.tar.gz> <output-dir>
# ARCH, x86_64 or arm64 (#190), must be the machine's own: flatpak-builder builds for the host, and
# the bundle is named koalino-ARCH-VERSION.flatpak.
# Needs: flatpak and flatpak-builder, with the org.freedesktop.Platform and Sdk 24.08 runtimes
# from Flathub (`flatpak install --user flathub org.freedesktop.Platform//24.08
# org.freedesktop.Sdk//24.08`). The bundle names Flathub as its runtime source, so
# `flatpak install koalino.flatpak` fetches the runtime when missing.
set -euo pipefail

tarball=$(realpath "$1")
output=$(realpath "$2")
here=$(dirname "$(realpath "$0")")
app_id=io.github.capannone_dev.koalino

# On disk, not in a RAM /tmp: the app and its models are over 1 GB unpacked.
work=$(mktemp -d -p "${TMPDIR:-/var/tmp}")
trap 'rm -rf "$work"' EXIT
cp "$here/$app_id.desktop" "$here/$app_id.metainfo.xml" "$here/koalino-192.png" "$here/koalino-128.png" "$here/koalino.svg" "$work/"
sed -e "s|@TARBALL@|$tarball|" -e "s|@SHA256@|$(sha256sum "$tarball" | cut -d' ' -f1)|" \
  "$here/$app_id.yml" > "$work/$app_id.yml"
flatpak-builder --user --force-clean --disable-rofiles-fuse --state-dir="$work/state" \
  --repo="$work/repo" \
  "$work/build" "$work/$app_id.yml" >/dev/null
mkdir -p "$output"
bundle="$output/$(basename "$tarball" .tar.gz | sed 's/^koalino-linux-/koalino-/').flatpak"
flatpak build-bundle --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
  "$work/repo" "$bundle" "$app_id"
echo "$bundle"
