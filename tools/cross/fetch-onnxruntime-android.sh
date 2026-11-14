#!/bin/bash
# Fetches Microsoft's ONNX Runtime for Android with the QNN provider (MIT; #75) and keeps only
# libonnxruntime.so, which the core loads at run time for diarization and the NPU. Qualcomm's QNN
# libraries are NOT fetched here: they are proprietary, so the app downloads them on the owner's
# request instead of shipping them.
set -euo pipefail
T=${NOTE_TAKER_TOOLCHAINS:-$HOME/toolchains}
VERSION=1.29.0
SHA256=cc4cfd72a47703aee65d8e6ce7448fa7488e3fff6556f9d6d354ec1687347cfd
mkdir -p "$T/onnxruntime-android"
curl -sSfL -o "$T/onnxruntime-android/ort.aar" \
  "https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android-qnn/$VERSION/onnxruntime-android-qnn-$VERSION.aar"
echo "$SHA256  $T/onnxruntime-android/ort.aar" | sha256sum -c -
unzip -q -o -j "$T/onnxruntime-android/ort.aar" jni/arm64-v8a/libonnxruntime.so -d "$T/onnxruntime-android"
rm "$T/onnxruntime-android/ort.aar"
echo "ONNX Runtime $VERSION for Android in $T/onnxruntime-android"
