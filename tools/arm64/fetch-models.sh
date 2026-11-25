#!/usr/bin/env bash
# Downloads the models every desktop package carries into a folder, checked against pinned
# hashes: Whisper base from whisper.cpp's own source (its published SHA-1), and the diarization
# models from this repository's models-v1 release (their SHA-256), as the macOS build does.
#
# Usage: tools/arm64/fetch-models.sh FOLDER
# Needs: curl, the GitHub CLI `gh` with a token (GH_TOKEN), sha1sum and sha256sum.
# Side effects: writes FOLDER/ggml-base.bin and FOLDER/diarization/{segmentation,embedding}.onnx;
# fails, leaving the files there, when a hash does not match.
set -euo pipefail
MODELS=${1:?usage: fetch-models.sh FOLDER}
REPOSITORY=${GITHUB_REPOSITORY:-capannone-dev/koalino}

mkdir -p "$MODELS/diarization"
curl -sSfL -o "$MODELS/ggml-base.bin" \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
echo "465707469ff3a37a2b9b8d8f89f2f99de7299dac  $MODELS/ggml-base.bin" | sha1sum -c -
gh release download models-v1 --repo "$REPOSITORY" --clobber \
  --pattern segmentation.onnx --pattern embedding.onnx --dir "$MODELS/diarization"
(cd "$MODELS/diarization" && sha256sum -c - <<'SUMS'
057ee564753071c0b09b5b611648b50ac188d50846bff5f01e9f7bbf1591ea25  segmentation.onnx
58d412cb009548f1816fd199d41b0d2bfe4f784a6836126fce94bfee820b7ed1  embedding.onnx
SUMS
)
