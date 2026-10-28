<!-- This directory is Flutter-generated Windows runner scaffolding for the Flutter client. -->

# Windows runner

Flutter owns the standard Windows runner and CMake bootstrap files. All application behavior
remains behind the Dart bridge interface.

## Building

Flutter cannot cross-compile a Windows app, so the runner is built on Windows; the Rust core is
cross-built on Linux.

1. On Linux: `tools/cross/build-flutter-native.sh windows` stages
   `ui/native/windows/note_taker_core.dll` (x86_64-pc-windows-gnu, release, whisper.cpp
   included). It imports only Windows system DLLs and the UCRT, so it needs no MinGW runtime.
2. Copy `ui/` to the Windows machine (for the test VM: `vm-deploy --vm win11 ui`).
3. On Windows, with Visual Studio 2022 Build Tools (workload
   `Microsoft.VisualStudio.Workload.VCTools`), Git, and Flutter 3.47.8:
   `flutter build windows --release`. `CMakeLists.txt` installs the staged DLL beside
   `note_taker_ui.exe` in `build\windows\x64\runner\Release\`; without it CMake warns and
   every native call fails.

The database key is kept in Windows Credential Manager (generic credential
`note-taker:encryption-key`).

## Background-control integration guide

Do not add C# or platform-specific service code to the Rust core. A Windows runner, tray host, or
future lock-screen/media-control integration must load the same Rust DLL and invoke the documented
no-argument FFI lifecycle commands (`start`, `pause`, `stop`, interruption notification, and state
query). Query `note_taker_get_current_state()` after every command, serialize visual updates on the
runner’s UI dispatcher, and never retain native pointers across callbacks. The Rust runtime mutex
serializes concurrent runner and UI calls; an Interrupted state is recoverable only through Start.
