// Has the Android host hand the Keystore's database key to the Rust core before it starts.
// Kotlin's `EncryptionKeyVault` unwraps the key with a non-exportable Keystore key and passes it
// to Rust over JNI (`NativeCore.provideEncryptionKey`), so the key never enters the Dart heap.
// A failure becomes the keychain error (status 6): there is no key-file fallback (see
// DECISION_LOG.md).

import 'package:flutter/services.dart' show MethodChannel;
import 'package:note_taker_ui/controllers/core_startup_controller.dart';

/// The platform channel `MainActivity` registers for the key vault.
const String keyVaultChannelName = 'note_taker/key_vault';

/// Asks the Android host to give the Rust core its database key.
final class AndroidKeyVault {
  /// Creates the vault client.
  const AndroidKeyVault();

  /// Has Kotlin unwrap the key for `keyDirectory` and pass it to Rust over JNI.
  ///
  /// Inputs: The app-private directory holding the wrapped key (and any old plain key file,
  /// which Kotlin wraps and deletes).
  /// Outputs: Completes once Rust holds the key. Throws `PlatformException` when the Keystore
  /// or the wrapped key is unusable (`keystore_unavailable`) or Rust refused the key
  /// (`key_rejected`).
  /// Side effects: May create the Keystore key and the wrapped key file on first start. No key
  /// bytes cross the channel.
  Future<void> provideEncryptionKey(String keyDirectory) =>
      const MethodChannel(keyVaultChannelName).invokeMethod<void>(
          'provideEncryptionKey',
          <String, Object?>{'keyDirectory': keyDirectory});
}

/// Starts the core, first having the host hand over its key when the platform keeps one.
///
/// Inputs: `provideHostKey` (null on platforms whose keychain Rust reaches itself; throws on
/// failure) and `startCore` (Rust's start status).
/// Outputs: `startCore`'s status, or `keychainUnavailableStatus` when the key cannot be handed
/// over, in which case the core is not started.
/// Side effects: Those of the two callbacks.
Future<int> startCoreWithHostKey({
  required Future<void> Function()? provideHostKey,
  required Future<int> Function() startCore,
}) async {
  if (provideHostKey != null) {
    try {
      await provideHostKey();
    } on Object {
      return keychainUnavailableStatus;
    }
  }
  return startCore();
}
