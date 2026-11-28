#!/usr/bin/env bash
# Builds an RPM package (.rpm) of the Koalino server from the server release tarball (#191), for
# Fedora, openSUSE and other RPM distributions. Same layout as build-server-deb.sh.
#
# Usage: build-server-rpm.sh <koalino-server-linux-x86_64-VERSION.tar.gz> <rpm-version> <output-dir>
#   e.g. build-server-rpm.sh koalino-server-linux-x86_64-0.2.0.tar.gz 0.2.0~preview.2 out/
# Needs: rpmbuild and file (Fedora `rpm-build`, Debian `rpm` and `file`). Installing creates the koalino user and the
# data folders but starts nothing.
# Side effects: writes the .rpm into <output-dir>; temporary files go under $TMPDIR (default
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
  koalino-server-linux-x86_64-*) rpm_arch=x86_64 ;;
  koalino-server-linux-arm64-*) rpm_arch=aarch64 ;;
  *) echo "cannot tell the architecture of $tarball (koalino-server-linux-x86_64|arm64-VERSION.tar.gz)" >&2; exit 1 ;;
esac

work=$(mktemp -d -p "${TMPDIR:-/var/tmp}")
trap 'rm -rf "$work"' EXIT
root=$work/root
mkdir -p "$root/opt/koalino-server" "$root/usr/share/doc/koalino-server" "$root/usr/share/licenses/koalino-server"
tar -xzf "$tarball" -C "$root/opt/koalino-server" --strip-components=1
"$here/server/install-files.sh" "$root"
install -m 644 "$repo/THIRD_PARTY_NOTICES.md" "$root/usr/share/doc/koalino-server/"
install -m 644 "$repo/LICENSE" "$root/usr/share/licenses/koalino-server/"

cat >"$work/koalino-server.spec" <<SPEC
Name:           koalino-server
Version:        $version
Release:        1
Summary:        Offline meeting notes server: transcribe, sync, web dashboard
License:        GPL-3.0-or-later
URL:            https://github.com/capannone-dev/koalino
# The programs bundle their own libraries and models; only system libraries are required by hand.
AutoReqProv:    no
Requires:       libstdc++, systemd
Recommends:     nodejs >= 20
Requires(pre):  systemd
Requires(post): systemd

%global debug_package %{nil}
%global __strip /bin/true
%global _build_id_links none

%description
The Koalino daemon (HTTP API, transcription with speaker names, encrypted sync with your Koalino
devices), its CLI and web dashboard, as systemd services running as the koalino user. Ships
Whisper base and the diarization models. Installing starts nothing: enable the service with
"systemctl enable --now koalino-daemon".

%install
cp -a "$root/." %{buildroot}/

%pre
# The user must exist before its files are installed with its group; the same lines as
# /usr/lib/sysusers.d/koalino.conf, which is not installed yet.
systemd-sysusers - <<'USERS'
u koalino - "Koalino server" /var/lib/koalino /usr/sbin/nologin
m koalino audio
USERS

%post
install -d -o koalino -g koalino -m 0750 /var/lib/koalino /var/log/koalino
if [ -d /run/systemd/system ]; then
  systemctl daemon-reload
  # An upgrade restarts what was running, on the new programs.
  for unit in koalino-daemon koalino-dashboard; do
    if systemctl is-active --quiet "\$unit"; then systemctl restart "\$unit"; fi
  done
fi
if [ "\$1" -eq 1 ]; then
  echo "Koalino server installed. Settings: /etc/koalino/server.env"
  echo "Start it with: sudo systemctl enable --now koalino-daemon"
fi

%preun
if [ "\$1" -eq 0 ] && [ -d /run/systemd/system ]; then
  systemctl disable --now koalino-dashboard koalino-daemon >/dev/null 2>&1 || true
fi

%postun
if [ -d /run/systemd/system ]; then systemctl daemon-reload || true; fi

%files
/opt/koalino-server
/usr/bin/koalino-cli
/usr/lib/systemd/system/koalino-daemon.service
/usr/lib/systemd/system/koalino-dashboard.service
/usr/lib/sysusers.d/koalino.conf
%dir %attr(0750, root, koalino) /etc/koalino
%config(noreplace) %attr(0640, root, koalino) /etc/koalino/server.env
%doc /usr/share/doc/koalino-server/THIRD_PARTY_NOTICES.md
%license /usr/share/licenses/koalino-server/LICENSE
SPEC

rpmbuild -bb --quiet --target "$rpm_arch" --define "_topdir $work/rpmbuild" --define "_rpmdir $output" \
  --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" "$work/koalino-server.spec"
echo "$output/koalino-server-$version-1.$rpm_arch.rpm"
