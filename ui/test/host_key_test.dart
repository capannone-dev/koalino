// Specifies how the app hands the Android Keystore's database key to Rust before starting the
// core, and that a Keystore failure becomes the keychain error (status 6), never a key file.

import 'dart:isolate';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/core_startup_controller.dart';
import 'package:note_taker_ui/infrastructure/host_key.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('startCoreWithHostKey', () {
    test('without a host key store it only starts the core', () async {
      final List<String> calls = <String>[];

      final int status = await startCoreWithHostKey(
        loadHostKey: null,
        provideKey: (TransferableTypedData key) async {
          calls.add('provide');
          return 0;
        },
        startCore: () async {
          calls.add('start');
          return 0;
        },
      );

      expect(status, 0);
      expect(calls, <String>['start']);
    });

    test('hands the key to Rust, wipes the Dart copy, then starts', () async {
      final Uint8List loadedKey = Uint8List.fromList(List<int>.filled(32, 7));
      Uint8List? receivedKey;

      final int status = await startCoreWithHostKey(
        loadHostKey: () async => loadedKey,
        provideKey: (TransferableTypedData key) async {
          receivedKey = key.materialize().asUint8List();
          return 0;
        },
        startCore: () async => receivedKey == null ? 99 : 0,
      );

      expect(status, 0);
      expect(receivedKey, List<int>.filled(32, 7));
      expect(loadedKey, List<int>.filled(32, 0),
          reason: 'the loaded copy is zero-filled');
    });

    test('a read-only key from the platform channel is still handed over',
        () async {
      final Uint8List channelKey =
          Uint8List.fromList(List<int>.filled(32, 4)).asUnmodifiableView();
      Uint8List? receivedKey;

      final int status = await startCoreWithHostKey(
        loadHostKey: () async => channelKey,
        provideKey: (TransferableTypedData key) async {
          receivedKey = key.materialize().asUint8List();
          return 0;
        },
        startCore: () async => 0,
      );

      expect(status, 0);
      expect(receivedKey, List<int>.filled(32, 4));
    });

    test('a Keystore failure is the keychain error and does not start',
        () async {
      bool started = false;

      final int status = await startCoreWithHostKey(
        loadHostKey: () async =>
            throw PlatformException(code: 'keystore_unavailable'),
        provideKey: (TransferableTypedData key) async => 0,
        startCore: () async {
          started = true;
          return 0;
        },
      );

      expect(status, keychainUnavailableStatus);
      expect(started, isFalse);
    });

    test('a key Rust refuses is the keychain error', () async {
      final int status = await startCoreWithHostKey(
        loadHostKey: () async => Uint8List(32),
        provideKey: (TransferableTypedData key) async => 1,
        startCore: () async => 0,
      );

      expect(status, keychainUnavailableStatus);
    });
  });

  test('AndroidKeyVault asks the Kotlin vault for the key of a directory',
      () async {
    const MethodChannel channel = MethodChannel(keyVaultChannelName);
    MethodCall? received;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (MethodCall call) async {
      received = call;
      return Uint8List.fromList(List<int>.filled(32, 5));
    });
    addTearDown(() => TestDefaultBinaryMessengerBinding
        .instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null));

    final Uint8List key =
        await const AndroidKeyVault().loadEncryptionKey('/data/app/keys');

    expect(received?.method, 'loadEncryptionKey');
    expect(received?.arguments,
        <String, Object?>{'keyDirectory': '/data/app/keys'});
    expect(key, List<int>.filled(32, 5));
  });
}
