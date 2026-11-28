<!-- This directory holds the files the Linux server packages (.deb, .rpm) install besides the server tarball. -->

# Linux server packaging files

`../build-server-deb.sh` and `../build-server-rpm.sh` (#191) turn the server tarball
(`koalino-server-linux-x86_64-VERSION.tar.gz`) into a `koalino-server` package. The programs go
to `/opt/koalino-server` as the tarball has them; the files here run them as a system service.

`assemble.sh` lays out the server folder that becomes the tarball (#194). It is shared by
`../build-server.sh` (native, in CI) and `tools/cross/package-linux-server.sh` (cross build on the
maintainer's machine), and is not installed.

| File | Installed as | What it does |
| --- | --- | --- |
| `koalino-daemon.service` | `/usr/lib/systemd/system/` | Runs the daemon as the `koalino` user, with the paths in `service.env` and the owner's settings in `/etc/koalino/server.env` |
| `koalino-dashboard.service` | `/usr/lib/systemd/system/` | Serves the web dashboard on `127.0.0.1:3000` (needs Node.js 20 or newer) |
| `koalino.sysusers` | `/usr/lib/sysusers.d/koalino.conf` | The `koalino` system user and group, home `/var/lib/koalino` |
| `service.env` | `/opt/koalino-server/service.env` | Where the service keeps its data: database, key file, logs, clips, exports. Not to be edited: the package replaces it |
| `server.env` | `/etc/koalino/server.env` (`root:koalino`, `0640`) | The owner's settings: passphrase, bind address, pairing name. Kept across upgrades |
| `koalino-cli` | `/usr/bin/koalino-cli` | Runs `cli` on the service's data; run it as the service user, `sudo -u koalino koalino-cli …` |

Installing starts nothing: the daemon listens for paired devices on the network, so the
administrator turns it on with `sudo systemctl enable --now koalino-daemon`. Removing the package
stops the services and keeps `/var/lib/koalino`, the meetings.
