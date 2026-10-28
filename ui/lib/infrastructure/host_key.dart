// Hands the Android Keystore's database key to the Rust core before it starts. Kotlin's
// `EncryptionKeyVault` unwraps the key with a non-exportable Keystore key; Rust receives it
// through `note_taker_provide_encryption_key` and serves it as its keychain. A failure becomes
// the keychain error (status 6): there is no key-file fallback (see DECISION_LOG.md).

import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:flutter/services.dart' show MethodChannel, PlatformException;
import 'package:note_taker_ui/controllers/core_startup_controller.dart';

/// The platform channel `MainActivity` registers for the key vault.
const String keyVaultChannelName = 'note_taker/key_vault';

/// Loads the database key from the Android Keystore through Kotlin.
final class AndroidKeyVault {
  /// Creates the vault client.
  const AndroidKeyVault();

  /// Returns the 32-byte database key for `keyDirectory`.
  ///
  /// Inputs: The app-private directory holding the wrapped key (and any old plain key file,
  /// which Kotlin wraps and deletes).
  /// Outputs: The key; the caller zero-fills it after use. Throws `PlatformException` when the
  /// Keystore or the wrapped key is unusable.
  /// Side effects: May create the Keystore key and the wrapped key file on first start.
  Future<Uint8List> loadEncryptionKey(String keyDirectory) async {
    final Uint8List? key = await const MethodChannel(keyVaultChannelName)
        .invokeMethod<Uint8List>('loadEncryptionKey',
            <String, Object?>{'keyDirectory': keyDirectory});
    if (key == null) {
      throw PlatformException(
          code: 'no_key', message: 'the vault returned no key');
    }
    return key;
  }
}

/// Starts the core, first handing it the host's key when the platform keeps one.
///
/// Inputs: `loadHostKey` (null on platforms whose keychain Rust reaches itself), `provideKey`
/// (hands the key to Rust; 0 on success), and `startCore` (Rust's start status).
/// Outputs: `startCore`'s status, or `keychainUnavailableStatus` when the key cannot be loaded
/// or handed over, in which case the core is not started.
/// Side effects: Zero-fills the loaded key buffer once it is handed over, when it is writable.
Future<int> startCoreWithHostKey({
  required Future<Uint8List> Function()? loadHostKey,
  required Future<int> Function(TransferableTypedData key) provideKey,
  required Future<int> Function() startCore,
}) async {
  if (loadHostKey != null) {
    final Uint8List key;
    try {
      key = await loadHostKey();
    } on Object {
      return keychainUnavailableStatus;
    }
    // Moves a copy to the isolate without another heap copy, then wipes this one if it can.
    final TransferableTypedData transferable =
        TransferableTypedData.fromList(<TypedData>[key]);
    _wipeIfWritable(key);
    if (await provideKey(transferable) != 0) {
      return keychainUnavailableStatus;
    }
  }
  return startCore();
}

/// Zero-fills `bytes` when the list is writable.
///
/// The platform channel delivers a read-only view; that buffer cannot be wiped from Dart and is
/// left to the garbage collector (Kotlin wipes its own copy after sending).
void _wipeIfWritable(Uint8List bytes) {
  try {
    bytes.fillRange(0, bytes.length, 0);
  } on UnsupportedError {
    // Read-only view from the channel codec.
  }
}

/// Calls `note_taker_provide_encryption_key` on a background isolate.
///
/// Inputs: The key, transferred to the isolate.
/// Outputs: Rust's status (0 stored, 1 invalid).
/// Side effects: Copies the key into native memory for the call and zero-fills both copies.
Future<int> provideEncryptionKeyToCore(TransferableTypedData key) =>
    Isolate.run(() {
      final Uint8List keyBytes = key.materialize().asUint8List();
      final Pointer<Uint8> nativeKey = calloc<Uint8>(keyBytes.length);
      try {
        nativeKey.asTypedList(keyBytes.length).setAll(0, keyBytes);
        final DynamicLibrary coreLibrary = Platform.isWindows
            ? DynamicLibrary.open('note_taker_core.dll')
            : DynamicLibrary.open('libnote_taker_core.so');
        return coreLibrary.lookupFunction<_NativeProvideKey, _DartProvideKey>(
            'note_taker_provide_encryption_key')(nativeKey, keyBytes.length);
      } finally {
        nativeKey.asTypedList(keyBytes.length).fillRange(0, keyBytes.length, 0);
        keyBytes.fillRange(0, keyBytes.length, 0);
        calloc.free(nativeKey);
      }
    });

/// Represents `int note_taker_provide_encryption_key(const uint8_t *key, size_t length)`.
typedef _NativeProvideKey = Int32 Function(Pointer<Uint8> key, Size length);

/// Represents the Dart callable form of `note_taker_provide_encryption_key`.
typedef _DartProvideKey = int Function(Pointer<Uint8> key, int length);
