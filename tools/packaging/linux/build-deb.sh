#!/usr/bin/env bash
# Builds a Debian package (.deb) of the Koalino desktop app from the Linux release tarball.
#
# Usage: build-deb.sh <koalino-linux-ARCH-VERSION.tar.gz> <deb-version> <output-dir>
#   e.g. build-deb.sh koalino-linux-x86_64-0.1.0.tar.gz 0.1.0~preview.1 out/
# ARCH, x86_64 or arm64 (#175), is read from the tarball's name and gives the package's
# architecture (amd64 or arm64).
# Needs: dpkg-deb (Debian or Ubuntu). The app goes to /opt/koalino, `koalino` to /usr/bin, and
# the menu entry, icon and AppStream file to /usr/share. Nothing is downloaded.
set -euo pipefail

tarball=$(realpath "$1")
version=$2
output=$(realpath "$3")
here=$(dirname "$(realpath "$0")")
app_id=io.github.capannone_dev.koalino
case $(basename "$tarball") in
  koalino-linux-arm64-*) deb_arch=arm64 ;;
  koalino-linux-x86_64-*) deb_arch=amd64 ;;
  *) echo "cannot tell the architecture of $tarball (koalino-linux-x86_64|arm64-VERSION.tar.gz)" >&2; exit 1 ;;
esac

# On disk, not in a RAM /tmp: the app and its models are over 1 GB unpacked.
work=$(mktemp -d -p "${TMPDIR:-/var/tmp}")
trap 'rm -rf "$work"' EXIT
root=$work/root
mkdir -p "$root/opt/koalino" "$root/usr/bin" "$root/DEBIAN" \
  "$root/usr/share/applications" "$root/usr/share/icons/hicolor/192x192/apps" \
  "$root/usr/share/metainfo" "$root/usr/share/doc/koalino"
tar -xzf "$tarball" -C "$root/opt/koalino" --strip-components=1
ln -s /opt/koalino/note_taker_ui "$root/usr/bin/koalino"
install -m 644 "$here/$app_id.desktop" "$root/usr/share/applications/"
install -m 644 "$here/koalino-192.png" "$root/usr/share/icons/hicolor/192x192/apps/$app_id.png"
install -Dm 644 "$here/koalino-128.png" "$root/usr/share/icons/hicolor/128x128/apps/$app_id.png"
install -Dm 644 "$here/koalino.svg" "$root/usr/share/icons/hicolor/scalable/apps/$app_id.svg"
install -m 644 "$here/$app_id.metainfo.xml" "$root/usr/share/metainfo/"
repo=$(realpath "$here/../../..")
install -m 644 "$repo/THIRD_PARTY_NOTICES.md" "$root/usr/share/doc/koalino/"
cat > "$root/usr/share/doc/koalino/copyright" <<'COPYRIGHT'
Koalino is free software, licensed under the GNU General Public License, version 3 or later
(/usr/share/common-licenses/GPL-3). Source: https://github.com/capannone-dev/koalino
The licenses of the libraries and models it ships are in THIRD_PARTY_NOTICES.md beside this
file and in /opt/koalino/data/models/diarization/licenses/.
COPYRIGHT

installed_kib=$(du -sk "$root" | cut -f1)
cat > "$root/DEBIAN/control" <<CONTROL
Package: koalino
Version: $version
Architecture: $deb_arch
Maintainer: Koalino project <https://github.com/capannone-dev/koalino/issues>
Installed-Size: $installed_kib
Depends: libgtk-3-0t64 | libgtk-3-0, libasound2t64 | libasound2, libstdc++6, xdg-utils
Recommends: gnome-keyring | kwalletmanager
Section: utils
Priority: optional
Homepage: https://github.com/capannone-dev/koalino
Description: Offline meeting notes: record, transcribe, summarize
 Koalino records meetings and transcribes them on your own devices, with
 speaker names, notes, photos and files. Devices sync with each other
 directly, encrypted, without a server. Ships Whisper base and the
 diarization models.
CONTROL

mkdir -p "$output"
deb="$output/koalino_${version}_${deb_arch}.deb"
dpkg-deb --root-owner-group -Zxz --build "$root" "$deb" >/dev/null
echo "$deb"
