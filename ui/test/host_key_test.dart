// Specifies how the app has Kotlin hand the Android Keystore's database key to Rust over JNI
// before starting the core, and that a failure becomes the keychain error (status 6), never a
// key file. The key itself never reaches Dart.

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
        provideHostKey: null,
        startCore: () async {
          calls.add('start');
          return 0;
        },
      );

      expect(status, 0);
      expect(calls, <String>['start']);
    });

    test('has the host hand the key over before starting', () async {
      final List<String> calls = <String>[];

      final int status = await startCoreWithHostKey(
        provideHostKey: () async => calls.add('provide'),
        startCore: () async {
          calls.add('start');
          return 0;
        },
      );

      expect(status, 0);
      expect(calls, <String>['provide', 'start']);
    });

    test('a Keystore failure is the keychain error and does not start',
        () async {
      bool started = false;

      final int status = await startCoreWithHostKey(
        provideHostKey: () async =>
            throw PlatformException(code: 'keystore_unavailable'),
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
        provideHostKey: () async =>
            throw PlatformException(code: 'key_rejected'),
        startCore: () async => 0,
      );

      expect(status, keychainUnavailableStatus);
    });
  });

  test('AndroidKeyVault asks Kotlin to hand over the key of a directory',
      () async {
    const MethodChannel channel = MethodChannel(keyVaultChannelName);
    MethodCall? received;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (MethodCall call) async {
      received = call;
      return null;
    });
    addTearDown(() => TestDefaultBinaryMessengerBinding
        .instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null));

    await const AndroidKeyVault().provideEncryptionKey('/data/app/keys');

    expect(received?.method, 'provideEncryptionKey');
    expect(received?.arguments,
        <String, Object?>{'keyDirectory': '/data/app/keys'});
  });
}
