#!/usr/bin/env bash
# Puts the server packages' own files into a package root (#191); shared by build-server-deb.sh
# and build-server-rpm.sh, so both packages install the same layout.
#
# Usage: install-files.sh <package-root>   (the tarball is already unpacked in /opt/koalino-server)
# Side effects: writes under <package-root> only.
set -euo pipefail
root=$1
here=$(dirname "$(realpath "$0")")
install -Dm 644 "$here/koalino-daemon.service" "$root/usr/lib/systemd/system/koalino-daemon.service"
install -Dm 644 "$here/koalino-dashboard.service" "$root/usr/lib/systemd/system/koalino-dashboard.service"
install -Dm 644 "$here/koalino.sysusers" "$root/usr/lib/sysusers.d/koalino.conf"
install -Dm 644 "$here/service.env" "$root/opt/koalino-server/service.env"
# Readable by the koalino group: the post-install script sets the group once the user exists.
install -Dm 640 "$here/server.env" "$root/etc/koalino/server.env"
install -Dm 755 "$here/koalino-cli" "$root/usr/bin/koalino-cli"
