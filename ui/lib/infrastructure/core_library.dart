// Opens the Rust core library for the platform the app runs on, so every bridge loads it the same
// way (before #151 each bridge named the file itself, and none knew macOS).
//
// Linux and Android load `libnote_taker_core.so` by name (the bundle's lib/ and the APK's native
// libraries are on the loader's path); Windows loads `note_taker_core.dll` beside the runner; macOS
// loads `libnote_taker_core.dylib` from the app bundle's Contents/Frameworks, by full path, because
// dlopen does not search the executable's @rpath for a bare file name.

import 'dart:ffi';
import 'dart:io';

/// The core library's file name on macOS, inside `Koalino.app/Contents/Frameworks`.
const String macosCoreLibraryName = 'libnote_taker_core.dylib';

/// Returns where the core library is on macOS, given the app's executable.
///
/// Inputs: The path of the running executable (`Koalino.app/Contents/MacOS/<name>`).
/// Outputs: `Koalino.app/Contents/Frameworks/libnote_taker_core.dylib`. Side effects: None.
String macosCoreLibraryPath(String executablePath) {
  final Directory contents = File(executablePath).parent.parent;
  return '${contents.path}/Frameworks/$macosCoreLibraryName';
}

/// Opens the core library for this platform.
///
/// Outputs: The loaded library. Throws `UnsupportedError` on a platform without a core build
/// (iOS, Fuchsia), and an `ArgumentError` from `DynamicLibrary.open` when the file is missing.
/// Side effects: Loads native code into the process (once; later calls reuse it).
DynamicLibrary openCoreLibrary() {
  if (Platform.isAndroid || Platform.isLinux) {
    return DynamicLibrary.open('libnote_taker_core.so');
  }
  if (Platform.isWindows) {
    return DynamicLibrary.open('note_taker_core.dll');
  }
  if (Platform.isMacOS) {
    return DynamicLibrary.open(
        macosCoreLibraryPath(Platform.resolvedExecutable));
  }
  throw UnsupportedError('The Rust core is not built for this platform.');
}
