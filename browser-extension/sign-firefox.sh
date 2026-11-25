#!/bin/bash
# Signs the Firefox build (dist/firefox) through Mozilla's add-ons site (addons.mozilla.org, AMO).
#
# Default: an unlisted version, signed at once, which release Firefox installs permanently while it
# stays off the public catalogue (#123). Writes the signed .xpi into dist/signed.
#
# --listed: submits the version to the public AMO catalogue (#127) instead. It sends the listing
# text and reviewer notes in amo-listing.json, and, because the build is bundled by esbuild, a zip
# of the extension's source (dist/koalino-source-<version>.zip: the tracked files plus
# package-lock.json) for Mozilla's reviewers. It does not wait for the review: the version appears
# on AMO once Mozilla approves it.
#
# The AMO API key is read from ~/.config/koalino/amo.env (AMO_JWT_ISSUER=…, AMO_JWT_SECRET=…, mode
# 600, never committed) or from KOALINO_AMO_ENV, and handed to web-ext through its environment
# variables, so it appears neither on the command line nor in the output. Each signing needs a
# version AMO has not signed before: raise "version" in build.mjs first.
#
# Usage: node build.mjs && ./sign-firefox.sh [--listed]
set -euo pipefail
cd "$(dirname "$0")"
env_file=${KOALINO_AMO_ENV:-$HOME/.config/koalino/amo.env}
if [ ! -r "$env_file" ]; then
  echo "missing $env_file (AMO_JWT_ISSUER=… and AMO_JWT_SECRET=…)" >&2
  exit 1
fi
if [ ! -f dist/firefox/manifest.json ]; then
  echo "missing dist/firefox: run node build.mjs first" >&2
  exit 1
fi
# The value after "=", without surrounding spaces or quotes (a pasted key often brings some along).
value_of() {
  sed -n "s/^$1=//p" "$env_file" | tr -d '\r' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' \
    -e 's/^["'"'"']//' -e 's/["'"'"']$//'
}
issuer=$(value_of AMO_JWT_ISSUER)
secret=$(value_of AMO_JWT_SECRET)
if [ -z "$issuer" ] || [ -z "$secret" ]; then
  echo "$env_file must set AMO_JWT_ISSUER and AMO_JWT_SECRET" >&2
  exit 1
fi
if [ "${1:-}" = "--listed" ]; then
  version=$(node -p 'require("./dist/firefox/manifest.json").version')
  source_zip=dist/koalino-source-$version.zip
  rm -f "$source_zip"
  # The tracked files as committed, plus the lockfile (ignored by git) so the build is reproducible.
  git archive --format=zip -o "$source_zip" HEAD .
  [ -f package-lock.json ] && python3 -c 'import sys, zipfile; zipfile.ZipFile(sys.argv[1], "a").write("package-lock.json")' "$source_zip"
  WEB_EXT_API_KEY=$issuer WEB_EXT_API_SECRET=$secret \
    npx web-ext sign --channel=listed --source-dir dist/firefox --artifacts-dir dist/signed \
    --amo-metadata amo-listing.json --upload-source-code "$source_zip" --approval-timeout 0
else
  WEB_EXT_API_KEY=$issuer WEB_EXT_API_SECRET=$secret \
    npx web-ext sign --channel=unlisted --source-dir dist/firefox --artifacts-dir dist/signed
fi
