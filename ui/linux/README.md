<!-- This directory is Flutter-generated Linux runner scaffolding for the Flutter client. -->

# Linux runner

Flutter owns the standard Linux runner and CMake bootstrap files. Flutter presentation code does
not call C++ or Rust types directly.

`CMakeLists.txt` installs `ui/native/linux/libnote_taker_core.so` into the bundle's `lib/`, where
the Dart bridges open it by name. Stage it first with `tools/cross/build-flutter-native.sh linux`
(the release core with whisper.cpp); without it the build warns and the app runs without a core.
