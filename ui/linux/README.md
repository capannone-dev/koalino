<!-- This directory is Flutter-generated Linux runner scaffolding for the Flutter client. -->

# Linux runner

Flutter owns the standard Linux runner and CMake bootstrap files. Flutter presentation code does
not call C++ or Rust types directly.

`CMakeLists.txt` installs `ui/native/linux/libnote_taker_core.so` into the bundle's `lib/`, where
the Dart bridges open it by name. Stage it first with `tools/cross/build-flutter-native.sh linux`
(the release core with whisper.cpp); without it the build warns and the app runs without a core.

`runner/my_application.cc` is Flutter's runner with two additions. It sets the window icon from
`data/koalino.png` next to the executable (installed from `runner/resources/koalino.png`, which
`branding/make-icons.sh` generates; #165). And the window's title bar follows
the desktop's light or dark style. GTK 3 draws the title bar itself on GNOME (Wayland) and ignores
the dark style, so the runner reads `org.freedesktop.appearance` `color-scheme` from the XDG
desktop portal, sets `gtk-application-prefer-dark-theme`, and follows the portal's
`SettingChanged` signal. Without a portal the title bar keeps GTK's default.
