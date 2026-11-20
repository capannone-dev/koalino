// Verifies the Android app hands its core configuration to Kotlin for the background job that
// runs a waiting after-meeting pass while the phone charges (#87).

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/background_core.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('the configuration and key directory reach the host', () async {
    final List<MethodCall> calls = <MethodCall>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
            const MethodChannel(backgroundCoreChannelName),
            (MethodCall call) async {
      calls.add(call);
      return null;
    });

    await rememberBackgroundCoreConfiguration(
      configurationJson: '{"data_directory":"/data/x"}',
      keyDirectory: '/data/x/keys',
    );

    expect(calls, hasLength(1));
    expect(calls.single.method, 'remember');
    expect(calls.single.arguments, <String, Object?>{
      'configuration': '{"data_directory":"/data/x"}',
      'keyDirectory': '/data/x/keys',
    });
  });

  test('a host without the channel does not fail the start', () async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
            const MethodChannel(backgroundCoreChannelName), null);

    await rememberBackgroundCoreConfiguration(
      configurationJson: '{}',
      keyDirectory: '/keys',
    );
  });
}
