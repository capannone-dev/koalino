#!/usr/bin/env bash
# Checks that a build transcribes on the machine it runs on: records whisper.cpp's 11-second
# `jfk.wav` sample into a throwaway meeting with the CLI and expects the known sentence in the
# transcript. The ARM64 workflow runs it on the ARM runners, where a crash or a wrong result
# would otherwise only show up on users' machines (#175, #176).
#
# Usage: tools/arm64/check-transcription.sh CLI MODEL
#   CLI: a `cli` built with `--features local-ai`; MODEL: ggml-base.bin.
# Needs: curl and sha256sum. The sample is downloaded from whisper.cpp's v1.8.2 tag and checked
# against its SHA-256.
# Side effects: writes a database and the sample in a temporary folder, removed on exit; fails
# when the transcript lacks the sentence.
set -euo pipefail
CLI=$(realpath "${1:?usage: check-transcription.sh CLI MODEL}")
MODEL=$(realpath "${2:?usage: check-transcription.sh CLI MODEL}")

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -sSfL -o "$work/jfk.wav" https://raw.githubusercontent.com/ggml-org/whisper.cpp/v1.8.2/samples/jfk.wav
echo "59dfb9a4acb36fe2a2affc14bacbee2920ff435cb13cc314a08c13f66ba7860e  $work/jfk.wav" | sha256sum -c --quiet -

# A throwaway database, sealed with a passphrase rather than the system keychain.
export NOTE_TAKER_DATABASE_PATH=$work/check.sqlite3 NOTE_TAKER_WHISPER_MODEL=$MODEL
export NOTE_TAKER_KEYCHAIN_ENABLED=false NOTE_TAKER_ENCRYPTION_PASSPHRASE=transcription-check-only
started=$(date +%s)
"$CLI" record start --meeting-id transcription-check --input "$work/jfk.wav" --language en 2>/dev/null | tail -1
echo "transcribed in $(($(date +%s) - started)) s"
transcript=$("$CLI" meetings view transcription-check)
echo "$transcript" | sed -n '/^Transcript:/,/^Attachments:/p'
if ! echo "$transcript" | grep -qi "ask not what your country can do for you"; then
  echo "the transcript lacks the sample's sentence" >&2
  exit 1
fi
