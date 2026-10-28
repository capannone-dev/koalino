// Specifies when the Android app lock locks and unlocks the app, and how the setting changes.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/app_lock_controller.dart';

import 'fake_app_lock_bridge.dart';

void main() {
  /// A controller over `bridge` whose clock the test moves forward.
  ({AppLockController controller, void Function(Duration) advance})
      controllerWithClock(FakeAppLockBridge bridge) {
    DateTime now = DateTime.utc(2026, 10, 28, 12);
    final AppLockController controller =
        AppLockController(appLockBridge: bridge, clock: () => now);
    return (
      controller: controller,
      advance: (Duration elapsed) => now = now.add(elapsed),
    );
  }

  group('at start-up', () {
    test('an enabled lock locks the app', () async {
      final AppLockController controller =
          AppLockController(appLockBridge: FakeAppLockBridge(enabled: true));

      await controller.initialize();

      expect(controller.phase, AppLockPhase.locked);
    });

    test('a disabled lock leaves the app open', () async {
      final AppLockController controller =
          AppLockController(appLockBridge: FakeAppLockBridge());

      expect(controller.phase, AppLockPhase.checking);
      await controller.initialize();

      expect(controller.phase, AppLockPhase.unlocked);
    });

    test('an enabled lock without a device screen lock leaves the app open',
        () async {
      // Removing the screen lock already needs the device credential, so this is no bypass,
      // and it avoids locking the owner out with no way to authenticate.
      final AppLockController controller = AppLockController(
          appLockBridge: FakeAppLockBridge(enabled: true, deviceSecure: false));

      await controller.initialize();

      expect(controller.phase, AppLockPhase.unlocked);
    });

    test('a status failure keeps the app locked', () async {
      final AppLockController controller = AppLockController(
          appLockBridge: FakeAppLockBridge(enabled: true)..statusFails = true);

      await controller.initialize();

      expect(controller.phase, AppLockPhase.locked);
      expect(controller.message, isNotNull);
    });
  });

  group('unlocking', () {
    test('a successful prompt unlocks', () async {
      final AppLockController controller =
          AppLockController(appLockBridge: FakeAppLockBridge(enabled: true));
      await controller.initialize();

      await controller.unlock();

      expect(controller.phase, AppLockPhase.unlocked);
    });

    test('a cancelled prompt stays locked without an error', () async {
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true)
        ..authenticationResults.add(false);
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);
      await controller.initialize();

      await controller.unlock();

      expect(controller.phase, AppLockPhase.locked);
      expect(controller.message, isNull);
    });

    test('a second tap while the prompt is open shows no second prompt',
        () async {
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true)
        ..pendingAuthentication = Completer<bool>();
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);
      await controller.initialize();

      final Future<void> first = controller.unlock();
      await controller.unlock();
      bridge.pendingAuthentication!.complete(true);
      await first;

      expect(bridge.authenticationCount, 1);
      expect(controller.phase, AppLockPhase.unlocked);
    });

    test('the automatic prompt runs once per lock', () async {
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true)
        ..authenticationResults.add(false);
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);
      await controller.initialize();

      await controller.promptIfPending();
      await controller.promptIfPending();

      expect(bridge.authenticationCount, 1);
    });
  });

  group('after leaving the app', () {
    test('returning within the grace period stays unlocked', () async {
      final (:controller, :advance) =
          controllerWithClock(FakeAppLockBridge(enabled: true));
      await controller.initialize();
      await controller.unlock();

      controller.appHidden();
      advance(lockAfterBackground - const Duration(seconds: 1));
      controller.appShown();

      expect(controller.phase, AppLockPhase.unlocked);
    });

    test('returning after the grace period locks', () async {
      final (:controller, :advance) =
          controllerWithClock(FakeAppLockBridge(enabled: true));
      await controller.initialize();
      await controller.unlock();

      controller.appHidden();
      advance(lockAfterBackground);
      controller.appShown();

      expect(controller.phase, AppLockPhase.locked);
    });

    test('the prompt hiding the app does not lock it again', () async {
      // The PIN screen is an activity of its own, so the app is hidden while it shows.
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true)
        ..pendingAuthentication = Completer<bool>();
      final (:controller, :advance) = controllerWithClock(bridge);
      await controller.initialize();

      final Future<void> unlocking = controller.unlock();
      controller.appHidden();
      advance(const Duration(minutes: 2));
      bridge.pendingAuthentication!.complete(true);
      await unlocking;
      controller.appShown();

      expect(controller.phase, AppLockPhase.unlocked);
    });

    test('a disabled lock never locks', () async {
      final (:controller, :advance) = controllerWithClock(FakeAppLockBridge());
      await controller.initialize();

      controller.appHidden();
      advance(const Duration(hours: 1));
      controller.appShown();

      expect(controller.phase, AppLockPhase.unlocked);
    });
  });

  group('the setting', () {
    test('turning it on asks to authenticate, then stores it', () async {
      final FakeAppLockBridge bridge = FakeAppLockBridge();
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);
      await controller.initialize();

      await controller.setEnabled(true);

      expect(bridge.authenticationCount, 1);
      expect(bridge.storedValues, <bool>[true]);
      expect(controller.status?.enabled, isTrue);
      expect(controller.phase, AppLockPhase.unlocked);
    });

    test('turning it off also asks to authenticate', () async {
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true);
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);
      await controller.initialize();
      await controller.unlock();

      await controller.setEnabled(false);

      expect(bridge.authenticationCount, 2);
      expect(bridge.storedValues, <bool>[false]);
      expect(controller.status?.enabled, isFalse);
    });

    test('a cancelled prompt leaves the setting unchanged', () async {
      final FakeAppLockBridge bridge = FakeAppLockBridge()
        ..authenticationResults.add(false);
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);
      await controller.initialize();

      await controller.setEnabled(true);

      expect(bridge.storedValues, isEmpty);
      expect(controller.status?.enabled, isFalse);
    });

    test('it cannot be turned on without a device screen lock', () async {
      final FakeAppLockBridge bridge = FakeAppLockBridge(deviceSecure: false);
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);
      await controller.initialize();

      await controller.setEnabled(true);

      expect(bridge.authenticationCount, 0);
      expect(bridge.storedValues, isEmpty);
    });
  });
}
