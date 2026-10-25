<!-- This directory is Flutter-generated Windows runner scaffolding for the Flutter client. -->

# Windows runner

Flutter owns the standard Windows runner and CMake bootstrap files. A future release-packaging
task will place the Rust DLL beside the executable; all application behavior remains behind the
Dart bridge interface.

## Background-control integration guide

Do not add C# or platform-specific service code to the Rust core. A Windows runner, tray host, or
future lock-screen/media-control integration must load the same Rust DLL and invoke the documented
no-argument FFI lifecycle commands (`start`, `pause`, `stop`, interruption notification, and state
query). Query `note_taker_get_current_state()` after every command, serialize visual updates on the
runner’s UI dispatcher, and never retain native pointers across callbacks. The Rust runtime mutex
serializes concurrent runner and UI calls; an Interrupted state is recoverable only through Start.
