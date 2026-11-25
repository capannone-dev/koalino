#!/usr/bin/env bash
# Builds an RPM package (.rpm) of the Koalino desktop app from the Linux release tarball (#164),
# for Fedora, openSUSE and other RPM distributions. Same layout as build-deb.sh.
#
# Usage: build-rpm.sh <koalino-linux-ARCH-VERSION.tar.gz> <rpm-version> <output-dir>
#   e.g. build-rpm.sh koalino-linux-x86_64-0.2.0.tar.gz 0.2.0~preview.2 out/
# ARCH, x86_64 or arm64 (#175), is read from the tarball's name and gives the package's
# architecture (x86_64 or aarch64).
# Needs: rpmbuild and file (Fedora `rpm-build`, Debian `rpm` and `file`). The app goes to
# /opt/koalino, `koalino` to /usr/bin, and the menu entry, icons and AppStream file to /usr/share.
# Nothing is downloaded.
# Side effects: writes the .rpm into <output-dir>; temporary files go under $TMPDIR (default
# /var/tmp) and are removed on exit.
set -euo pipefail

tarball=$(realpath "$1")
version=$2
output=$(realpath "$3")
here=$(dirname "$(realpath "$0")")
repo=$(realpath "$here/../../..")
app_id=io.github.capannone_dev.koalino
case $(basename "$tarball") in
  koalino-linux-arm64-*) rpm_arch=aarch64 ;;
  koalino-linux-x86_64-*) rpm_arch=x86_64 ;;
  *) echo "cannot tell the architecture of $tarball (koalino-linux-x86_64|arm64-VERSION.tar.gz)" >&2; exit 1 ;;
esac

# On disk, not in a RAM /tmp: the app and its models are over 1 GB unpacked.
work=$(mktemp -d -p "${TMPDIR:-/var/tmp}")
trap 'rm -rf "$work"' EXIT
root=$work/root
mkdir -p "$root/opt/koalino" "$root/usr/bin" "$root/usr/share/applications" \
  "$root/usr/share/metainfo" "$root/usr/share/doc/koalino" "$root/usr/share/licenses/koalino"
tar -xzf "$tarball" -C "$root/opt/koalino" --strip-components=1
ln -s /opt/koalino/note_taker_ui "$root/usr/bin/koalino"
install -m 644 "$here/$app_id.desktop" "$root/usr/share/applications/"
install -Dm 644 "$here/koalino-192.png" "$root/usr/share/icons/hicolor/192x192/apps/$app_id.png"
install -Dm 644 "$here/koalino-128.png" "$root/usr/share/icons/hicolor/128x128/apps/$app_id.png"
install -Dm 644 "$here/koalino.svg" "$root/usr/share/icons/hicolor/scalable/apps/$app_id.svg"
install -m 644 "$here/$app_id.metainfo.xml" "$root/usr/share/metainfo/"
install -m 644 "$repo/THIRD_PARTY_NOTICES.md" "$root/usr/share/doc/koalino/"
install -m 644 "$repo/LICENSE" "$root/usr/share/licenses/koalino/"

cat >"$work/koalino.spec" <<SPEC
Name:           koalino
Version:        $version
Release:        1
Summary:        Offline meeting notes: record, transcribe, summarize
License:        GPL-3.0-or-later
URL:            https://github.com/capannone-dev/koalino
# The release bundles its own Flutter engine, core and models: scanning them would declare the
# bundled libraries as system-wide provides, so only the system libraries are required by hand.
AutoReqProv:    no
Requires:       gtk3, alsa-lib, libstdc++, xdg-utils
Recommends:     gnome-keyring

%global debug_package %{nil}
%global __strip /bin/true
%global _build_id_links none

%description
Koalino records meetings and transcribes them on your own devices, with speaker names, notes,
photos and files. Devices sync with each other directly, encrypted, without a server. Ships
Whisper base and the diarization models.

%install
cp -a "$root/." %{buildroot}/

%files
/opt/koalino
/usr/bin/koalino
/usr/share/applications/$app_id.desktop
/usr/share/icons/hicolor/192x192/apps/$app_id.png
/usr/share/icons/hicolor/128x128/apps/$app_id.png
/usr/share/icons/hicolor/scalable/apps/$app_id.svg
/usr/share/metainfo/$app_id.metainfo.xml
%doc /usr/share/doc/koalino/THIRD_PARTY_NOTICES.md
%license /usr/share/licenses/koalino/LICENSE
SPEC

# --target, not BuildArch, sets the architecture: BuildArch would make rpmbuild refuse to pack
# the ARM64 build on an x86_64 machine, though nothing is compiled.
rpmbuild -bb --quiet --target "$rpm_arch" --define "_topdir $work/rpmbuild" --define "_rpmdir $output" \
  --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" "$work/koalino.spec"
echo "$output/koalino-$version-1.$rpm_arch.rpm"
