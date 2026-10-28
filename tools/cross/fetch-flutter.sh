#!/bin/bash
# Installs the Flutter SDK (the version CI uses), a JDK and the Android SDK command-line tools into
# $NOTE_TAKER_TOOLCHAINS (default ~/toolchains), for building the Flutter app for Linux and Android.
set -euo pipefail
T=${NOTE_TAKER_TOOLCHAINS:-$HOME/toolchains}
FLUTTER_VERSION=3.47.8 JDK_MAJOR=21 ANDROID_CMDLINE_TOOLS=13114758
mkdir -p "$T/android-sdk/cmdline-tools"

curl -sSfL "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_$FLUTTER_VERSION-stable.tar.xz" \
  | tar -xJ -C "$T"
echo "flutter $FLUTTER_VERSION"

mkdir -p "$T/jdk" && curl -sSfL "https://api.adoptium.net/v3/binary/latest/$JDK_MAJOR/ga/linux/x64/jdk/hotspot/normal/eclipse" \
  | tar -xz -C "$T/jdk" --strip-components=1
echo "jdk $("$T/jdk/bin/java" -version 2>&1 | head -1)"

curl -sSfL "https://dl.google.com/android/repository/commandlinetools-linux-${ANDROID_CMDLINE_TOOLS}_latest.zip" -o "$T/cmdline-tools.zip"
(cd "$T/android-sdk/cmdline-tools" && unzip -q -o "$T/cmdline-tools.zip" && rm -rf latest && mv cmdline-tools latest && rm "$T/cmdline-tools.zip")
export JAVA_HOME=$T/jdk ANDROID_HOME=$T/android-sdk
(yes || true) | "$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" --licenses >/dev/null
# The versions Flutter 3.47.8's Gradle plugin compiles against (compileSdk 36, its default NDK).
"$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager" "platform-tools" "platforms;android-36" \
  "build-tools;36.0.0" "ndk;28.2.13676358" >/dev/null
echo "android sdk command-line tools $ANDROID_CMDLINE_TOOLS"

# The Flutter Linux desktop build needs ninja (GTK 3 headers come from the system).
"$T/cmake-venv/bin/pip" install -q ninja 2>/dev/null || echo "run fetch.sh first for the cmake venv"
