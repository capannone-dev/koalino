// Specifies Settings → Developer (#187): tapping the version seven times unlocks a section where
// an expert changes the app's variables; values are validated by the core, saved for the next
// start, and can be reset or turned off.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/developer_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/developer_settings_bridge.dart';
import 'package:note_taker_ui/presentation/developer_settings_screen.dart';

/// Answers like the core, keeping the state in memory.
final class FakeDeveloperSettingsBridge implements DeveloperSettingsBridge {
  bool unlocked = false;
  final Map<String, String> saved = <String, String>{};
  final Map<String, String> atStart = <String, String>{};
  final List<String> commands = <String>[];

  DeveloperSettingsState _state() => DeveloperSettingsState(
        unlocked: unlocked,
        restartNeeded: saved.toString() != atStart.toString(),
        settings: <DeveloperSetting>[
          DeveloperSetting(
            name: 'NOTE_TAKER_VAD_ENABLED',
            kind: DeveloperSettingKind.flag,
            description: 'Cuts the audio at pauses in speech.',
            defaultValue: 'true',
            saved: saved['NOTE_TAKER_VAD_ENABLED'],
            inEffect: null,
          ),
          DeveloperSetting(
            name: 'NOTE_TAKER_TEAMS_BRIDGE_PORT',
            kind: DeveloperSettingKind.integer,
            description: 'The bridge port.',
            defaultValue: '8765',
            min: 1024,
            max: 65535,
            saved: saved['NOTE_TAKER_TEAMS_BRIDGE_PORT'],
            inEffect: '8765',
          ),
        ],
      );

  @override
  Future<DeveloperSettingsState> get() async {
    commands.add('get');
    return _state();
  }

  @override
  Future<DeveloperSettingsState> unlock() async {
    commands.add('unlock');
    unlocked = true;
    return _state();
  }

  @override
  Future<DeveloperSettingsState> lock() async {
    commands.add('lock');
    unlocked = false;
    saved.clear();
    return _state();
  }

  @override
  Future<DeveloperSettingsState> reset() async {
    commands.add('reset');
    saved.clear();
    return _state();
  }

  @override
  Future<DeveloperSettingsState> set(String name, String? value) async {
    commands.add('set $name=$value');
    if (value == null) {
      saved.remove(name);
    } else if (name == 'NOTE_TAKER_TEAMS_BRIDGE_PORT' &&
        (int.tryParse(value) ?? 0) < 1024) {
      throw const DeveloperSettingsException(
          'invalid_value', 'use a whole number from 1024 to 65535');
    } else {
      saved[name] = value;
    }
    return _state();
  }
}

Future<DeveloperSettingsController> _pumpSettings(
    WidgetTester tester, FakeDeveloperSettingsBridge bridge) async {
  final DeveloperSettingsController controller =
      DeveloperSettingsController(bridge: bridge);
  await controller.load();
  await tester.pumpWidget(MaterialApp(
    home: Scaffold(
      body: ListView(children: <Widget>[
        DeveloperSettingsTile(controller: controller),
        DeveloperVersionTile(controller: controller, version: '0.2.0'),
      ]),
    ),
  ));
  return controller;
}

Future<void> _tapVersion(WidgetTester tester, int times) async {
  for (int tap = 0; tap < times; tap++) {
    await tester.tap(find.byKey(developerVersionTileKey));
    await tester.pump(const Duration(milliseconds: 100));
  }
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('seven taps on the version unlock the Developer entry',
      (WidgetTester tester) async {
    final FakeDeveloperSettingsBridge bridge = FakeDeveloperSettingsBridge();
    await _pumpSettings(tester, bridge);
    expect(find.text('Version 0.2.0'), findsOneWidget);
    expect(find.byKey(developerSettingsTileKey), findsNothing);

    await _tapVersion(tester, 6);
    expect(bridge.unlocked, isFalse);
    // The countdown is in the row, not over it.
    expect(
        find.text('1 more tap to turn on developer settings'), findsOneWidget);
    expect(find.byType(SnackBar), findsNothing);
    expect(find.byKey(developerSettingsTileKey), findsNothing);

    await _tapVersion(tester, 1);
    expect(bridge.commands, contains('unlock'));
    expect(find.byKey(developerSettingsTileKey), findsOneWidget);
  });

  testWidgets('a flag is saved, and a restart is asked for',
      (WidgetTester tester) async {
    final FakeDeveloperSettingsBridge bridge = FakeDeveloperSettingsBridge()
      ..unlocked = true;
    final DeveloperSettingsController controller =
        await _pumpSettings(tester, bridge);
    await tester.tap(find.byKey(developerSettingsTileKey));
    await tester.pumpAndSettle();
    expect(find.text('NOTE_TAKER_VAD_ENABLED'), findsOneWidget);
    expect(find.byKey(restartNeededKey), findsNothing);

    await tester.tap(find.text('NOTE_TAKER_VAD_ENABLED'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('false'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(saveDeveloperValueKey));
    await tester.pumpAndSettle();

    expect(bridge.saved['NOTE_TAKER_VAD_ENABLED'], 'false');
    expect(controller.state?.restartNeeded, isTrue);
    expect(find.byKey(restartNeededKey), findsOneWidget);
  });

  testWidgets('a refused value keeps the dialog open with the reason',
      (WidgetTester tester) async {
    final FakeDeveloperSettingsBridge bridge = FakeDeveloperSettingsBridge()
      ..unlocked = true;
    await _pumpSettings(tester, bridge);
    await tester.tap(find.byKey(developerSettingsTileKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('NOTE_TAKER_TEAMS_BRIDGE_PORT'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(developerValueFieldKey), '80');
    await tester.tap(find.byKey(saveDeveloperValueKey));
    await tester.pumpAndSettle();

    expect(find.byType(AlertDialog), findsOneWidget);
    expect(find.textContaining('1024 to 65535'), findsOneWidget);
    expect(bridge.saved, isEmpty);

    await tester.enterText(find.byKey(developerValueFieldKey), '9000');
    await tester.tap(find.byKey(saveDeveloperValueKey));
    await tester.pumpAndSettle();
    expect(bridge.saved['NOTE_TAKER_TEAMS_BRIDGE_PORT'], '9000');
  });

  testWidgets('Use default clears one value; turning off hides the entry',
      (WidgetTester tester) async {
    final FakeDeveloperSettingsBridge bridge = FakeDeveloperSettingsBridge()
      ..unlocked = true
      ..saved['NOTE_TAKER_VAD_ENABLED'] = 'false';
    await _pumpSettings(tester, bridge);
    await tester.tap(find.byKey(developerSettingsTileKey));
    await tester.pumpAndSettle();

    await tester.tap(find.text('NOTE_TAKER_VAD_ENABLED'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(useDefaultDeveloperValueKey));
    await tester.pumpAndSettle();
    expect(bridge.commands, contains('set NOTE_TAKER_VAD_ENABLED=null'));

    await tester.tap(find.byKey(turnOffDeveloperSettingsKey));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(confirmTurnOffDeveloperSettingsKey));
    await tester.pumpAndSettle();
    expect(bridge.commands, contains('lock'));
    expect(find.byKey(developerSettingsTileKey), findsNothing);
  });

  test('the core answer decodes into settings', () {
    final DeveloperSettingsState state = decodeDeveloperSettings(
      '{"ok":true,"unlocked":true,"restart_needed":false,"settings":['
      '{"name":"NOTE_TAKER_WHISPER_GPU","kind":"choice","options":["on","off"],'
      '"description":"GPU.","default":"on","saved":"off","in_effect":null}]}',
    );
    expect(state.unlocked, isTrue);
    final DeveloperSetting gpu = state.settings.single;
    expect(gpu.kind, DeveloperSettingKind.choice);
    expect(gpu.options, <String>['on', 'off']);
    expect(gpu.saved, 'off');
    expect(gpu.inEffect, isNull);
    expect(
      () => decodeDeveloperSettings(
          '{"ok":false,"error_code":"locked","error":"locked"}'),
      throwsA(isA<DeveloperSettingsException>()
          .having((DeveloperSettingsException e) => e.code, 'code', 'locked')),
    );
  });
}
