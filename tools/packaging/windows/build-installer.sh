#!/usr/bin/env bash
# Builds Koalino's Windows installer (koalino-windows-x86_64-VERSION-setup.exe) from the Windows
# release zip, with NSIS's makensis, which runs on Linux. Only repackages the release build:
# nothing is compiled or downloaded.
#
# Usage: build-installer.sh <koalino-windows-x86_64-VERSION.zip> <VERSION> <output-dir>
#   e.g. build-installer.sh koalino-windows-x86_64-0.2.0.zip 0.2.0 out/
# Needs: makensis (Debian/Ubuntu package `nsis`), unzip or python3.
# Side effects: writes <output-dir>/koalino-windows-x86_64-VERSION-setup.exe; unpacks the zip in
# a temporary folder that is removed on exit.
set -euo pipefail

zip=$(realpath "$1")
version=$2
mkdir -p "$3"
output=$(realpath "$3")
here=$(dirname "$(realpath "$0")")
repo=$(realpath "$here/../../..")

work=$(mktemp -d -p "${TMPDIR:-/var/tmp}")
trap 'rm -rf "$work"' EXIT
if command -v unzip >/dev/null; then
  unzip -q "$zip" -d "$work"
else
  python3 -I -c 'import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])' "$zip" "$work"
fi
source_dir=$(find "$work" -mindepth 1 -maxdepth 1 -type d | head -n 1)
[ -f "$source_dir/note_taker_ui.exe" ] || { echo "no note_taker_ui.exe in $zip" >&2; exit 1; }

# NSIS shows the license page as plain text with Windows line endings.
sed 's/$/\r/' "$repo/LICENSE" >"$work/LICENSE.txt"

installer=$output/koalino-windows-x86_64-$version-setup.exe
makensis -V2 \
  -DVERSION="$version" \
  -DSOURCE_DIR="$source_dir" \
  -DOUTPUT="$installer" \
  -DICON="$repo/ui/windows/runner/resources/app_icon.ico" \
  -DLICENSE_FILE="$work/LICENSE.txt" \
  "$here/koalino.nsi"
echo "installer: $installer"
