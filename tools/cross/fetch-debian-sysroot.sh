#!/bin/bash
# Installs a Debian 13 (trixie) sysroot into $NOTE_TAKER_TOOLCHAINS/debian-13-sysroot (default
# ~/toolchains), from packages pinned by hash. Linux builds link against it, so that they run on
# Debian 13 and on anything newer, instead of needing the build machine's glibc.
#
# A Debian point release can remove a pinned version from the mirror. If a download fails, take the
# new path and hash from dists/trixie/main/binary-amd64/Packages.
set -euo pipefail
T=${NOTE_TAKER_TOOLCHAINS:-$HOME/toolchains}
MIRROR=https://deb.debian.org/debian
SYSROOT=$T/debian-13-sysroot
DOWNLOADS=$(mktemp -d)
trap 'rm -rf "$DOWNLOADS"' EXIT

# Each line is a pool path and its SHA-256.
PACKAGES="
pool/main/g/glibc/libc6_2.41-12+deb13u4_amd64.deb 967aa62605721081c3eb2a17650611a792aa802d76a6511d1840242623d204c9
pool/main/g/glibc/libc6-dev_2.41-12+deb13u4_amd64.deb 1fda734dabcd80b77266a09ab62b0f1e3e16d8091db62899890b2200745632a2
pool/main/g/gcc-14/libgcc-s1_14.2.0-19_amd64.deb 3c71917b490d1a17aed43196a2787a256ecf060526cdb20216a74bedc061b150
pool/main/g/gcc-14/libgcc-14-dev_14.2.0-19_amd64.deb ca6f2d36d96b19b3eb71405b0b80134d8c89380b02204a2512e5c58ceb090628
pool/main/g/gcc-14/libstdc++6_14.2.0-19_amd64.deb ab1fa05837aa7a92aae748fd07a18a35f7d18bb4a71c4724fe2bbf0e32089de0
pool/main/g/gcc-14/libstdc++-14-dev_14.2.0-19_amd64.deb 4b962fac5f1af0bd8b1b3f97e0f47b9d8e9a79e88802143f00ebfbd78ad87a7b
pool/main/l/linux/linux-libc-dev_6.12.107-1_all.deb dc68dd53c7356598d005866da0d19d7bd33af2a62971aced624cdf04492902b4
pool/main/a/alsa-lib/libasound2t64_1.2.14-1+deb13u1_amd64.deb 5495496142d57e5ad6d581ac5456ec3c87f872c109a2bfb815d154b673eebc67
pool/main/a/alsa-lib/libasound2-dev_1.2.14-1+deb13u1_amd64.deb dfa8e8a133ef8704093d03464d86a3e86d26232baf1e3827aa030cb68c437e1c
"

rm -rf "$SYSROOT"
mkdir -p "$SYSROOT/usr/lib" "$SYSROOT/usr/lib64" "$SYSROOT/usr/bin"
# Debian's merged layout, which the base system normally sets up. libc.so names /lib paths.
ln -s usr/lib "$SYSROOT/lib"
ln -s usr/lib64 "$SYSROOT/lib64"
ln -s usr/bin "$SYSROOT/bin"
while read -r pool_path sha256; do
  [ -n "$pool_path" ] || continue
  package=$DOWNLOADS/$(basename "$pool_path")
  curl -sSfL -o "$package" "$MIRROR/$pool_path"
  echo "$sha256  $package" | sha256sum -c --quiet -
  # A .deb is an ar archive holding the file tree as data.tar.*.
  bsdtar -xOf "$package" 'data.tar.*' | bsdtar -xf - -C "$SYSROOT"
done <<<"$PACKAGES"
echo "Debian 13 sysroot in $SYSROOT"
