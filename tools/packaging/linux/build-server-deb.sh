#!/usr/bin/env bash
# Builds a Debian package (.deb) of the Koalino server from the server release tarball (#191).
#
# Usage: build-server-deb.sh <koalino-server-linux-x86_64-VERSION.tar.gz> <deb-version> <output-dir>
#   e.g. build-server-deb.sh koalino-server-linux-x86_64-0.2.0.tar.gz 0.2.0~preview.2 out/
# Needs: dpkg-deb. The programs and models go to /opt/koalino-server as the tarball has them;
# server/ adds the systemd units, the koalino system user, the settings file and koalino-cli.
# Installing creates the user and the data folders but starts nothing.
# Side effects: writes the .deb into <output-dir>; temporary files go under $TMPDIR (default
# /var/tmp) and are removed on exit. Nothing is downloaded.
set -euo pipefail
# Package files get 644/755 whatever the caller's umask.
umask 022

tarball=$(realpath "$1")
version=$2
output=$(realpath "$3")
here=$(dirname "$(realpath "$0")")
repo=$(realpath "$here/../../..")
case $(basename "$tarball") in
  koalino-server-linux-x86_64-*) deb_arch=amd64 ;;
  koalino-server-linux-arm64-*) deb_arch=arm64 ;;
  *) echo "cannot tell the architecture of $tarball (koalino-server-linux-x86_64|arm64-VERSION.tar.gz)" >&2; exit 1 ;;
esac

work=$(mktemp -d -p "${TMPDIR:-/var/tmp}")
trap 'rm -rf "$work"' EXIT
root=$work/root
mkdir -p "$root/opt/koalino-server" "$root/DEBIAN" "$root/usr/share/doc/koalino-server"
tar -xzf "$tarball" -C "$root/opt/koalino-server" --strip-components=1
"$here/server/install-files.sh" "$root"
install -m 644 "$repo/THIRD_PARTY_NOTICES.md" "$root/usr/share/doc/koalino-server/"
cat > "$root/usr/share/doc/koalino-server/copyright" <<'COPYRIGHT'
Koalino is free software, licensed under the GNU General Public License, version 3 or later
(/usr/share/common-licenses/GPL-3). Source: https://github.com/capannone-dev/koalino
The licenses of the libraries and models it ships are in THIRD_PARTY_NOTICES.md beside this
file and in /opt/koalino-server/models/diarization/licenses/.
COPYRIGHT
echo /etc/koalino/server.env > "$root/DEBIAN/conffiles"

installed_kib=$(du -sk "$root" | cut -f1)
cat > "$root/DEBIAN/control" <<CONTROL
Package: koalino-server
Version: $version
Architecture: $deb_arch
Maintainer: Koalino project <https://github.com/capannone-dev/koalino/issues>
Installed-Size: $installed_kib
Depends: libstdc++6, systemd
Recommends: nodejs (>= 20)
Section: utils
Priority: optional
Homepage: https://github.com/capannone-dev/koalino
Description: Offline meeting notes server: transcribe, sync, web dashboard
 The Koalino daemon (HTTP API, transcription with speaker names, encrypted
 sync with your Koalino devices), its CLI and web dashboard, as systemd
 services running as the koalino user. Ships Whisper base and the
 diarization models. Installing starts nothing: enable the service with
 "systemctl enable --now koalino-daemon".
CONTROL
for script in postinst prerm postrm; do
  install -m 755 "$here/server/deb-$script" "$root/DEBIAN/$script"
done

mkdir -p "$output"
deb="$output/koalino-server_${version}_${deb_arch}.deb"
dpkg-deb --root-owner-group -Zxz --build "$root" "$deb" >/dev/null
echo "$deb"
