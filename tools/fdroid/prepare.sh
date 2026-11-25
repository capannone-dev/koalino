#!/usr/bin/env bash
# Prepares a checkout for the F-Droid build of Koalino (#163); F-Droid's recipe runs it before
# `flutter pub get`. It removes what F-Droid cannot build from source:
#   - `pdfrx`, whose build hook downloads a prebuilt PDFium library: its view is replaced by
#     tools/fdroid/pdf_view.dart, which says PDFs cannot be shown in the app yet;
#   - the release signing fallback: the marker `ui/android/fdroid-unsigned` makes Gradle leave the
#     release APK unsigned, since F-Droid signs it with its own key.
# Usage: tools/fdroid/prepare.sh   Side effects: edits ui/pubspec.yaml and ui/lib in place, so run
# it only on a throwaway checkout.
set -euo pipefail
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cp "$repo/tools/fdroid/pdf_view.dart" "$repo/ui/lib/presentation/pdf_view.dart"
sed -i '/^  pdfrx:/d' "$repo/ui/pubspec.yaml"
if grep -rq "package:pdfrx" "$repo/ui/lib"; then
  echo "ui/lib still imports pdfrx outside pdf_view.dart" >&2
  exit 1
fi
touch "$repo/ui/android/fdroid-unsigned"
echo "prepared for the F-Droid build"
