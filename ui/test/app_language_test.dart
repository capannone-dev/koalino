// Specifies the app's languages (Issue #124): every translation has every message with the same
// placeholders, the choice is stored and read back, Settings offers it, and choosing a language
// changes the text already on screen, Arabic laying it out right to left.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/app_language_controller.dart';
import 'package:note_taker_ui/controllers/log_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/app_language_store.dart';
import 'package:note_taker_ui/l10n/l10n.dart';
import 'package:note_taker_ui/presentation/app_localization_scope.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';

import 'fake_log_bridge.dart';

/// Keeps the choice in memory.
final class _MemoryStore implements AppLanguageStore {
  _MemoryStore([this.stored]);

  String? stored;

  @override
  Future<String?> read() async => stored;

  @override
  Future<void> write(String? code) async => stored = code;
}

/// The ARB files' messages by locale, without the `@` metadata.
Map<String, Map<String, dynamic>> _arbs() => <String, Map<String, dynamic>>{
      for (final FileSystemEntity file in Directory('lib/l10n').listSync())
        if (file.path.endsWith('.arb'))
          (jsonDecode((file as File).readAsStringSync())
                  as Map<String, dynamic>)['@@locale'] as String:
              jsonDecode(file.readAsStringSync()) as Map<String, dynamic>,
    };

/// Every `{name` a message contains: its placeholders, plus any plural
/// branch whose text is a single word.
Set<String> _placeholders(String message) => <String>{
      for (final RegExpMatch match
          in RegExp(r'\{([A-Za-z0-9_]+)[,}]').allMatches(message))
        match.group(1)!,
    };

void main() {
  tearDown(
      () => useAppLocalizations(lookupAppLocalizations(const Locale('en'))));

  test('every language has every message with the same placeholders', () {
    final Map<String, Map<String, dynamic>> arbs = _arbs();
    expect(arbs.keys.toSet(), <String>{
      for (final AppLanguage language in appLanguages) language.code
    });
    final Map<String, dynamic> english = arbs['en']!;
    final Iterable<String> keys =
        english.keys.where((String key) => !key.startsWith('@'));
    for (final MapEntry<String, Map<String, dynamic>> arb in arbs.entries) {
      for (final String key in keys) {
        final Object? message = arb.value[key];
        expect(message, isA<String>(), reason: '${arb.key} lacks $key');
        final Set<String> declared = <String>{
          ...(((english['@$key'] as Map<String, dynamic>?)?['placeholders']
                      as Map<String, dynamic>?) ??
                  const <String, dynamic>{})
              .keys,
        };
        // Plural branch text such as `=1{recording}` is not a placeholder,
        // so only the declared names are compared.
        expect(
            _placeholders(message! as String).intersection(declared), declared,
            reason: '${arb.key} $key');
        expect((message as String).trim(), isNotEmpty,
            reason: '${arb.key} $key');
      }
      expect(
          arb.value.keys.where((String key) =>
              !key.startsWith('@') && !english.containsKey(key)),
          isEmpty,
          reason: '${arb.key} has messages English lacks');
    }
  });

  test('the choice is read back, and unknown codes follow the system',
      () async {
    final _MemoryStore store = _MemoryStore('it');
    final AppLanguageController controller =
        AppLanguageController(store: store);
    await controller.load();
    expect(controller.locale, const Locale('it'));

    await controller.choose('xx');
    expect(controller.languageCode, 'it');

    await controller.choose(null);
    expect(store.stored, isNull);
    expect(controller.locale, isNull);

    final AppLanguageController unknown =
        AppLanguageController(store: _MemoryStore('xx'));
    await unknown.load();
    expect(unknown.languageCode, isNull);
  });

  test('the file store keeps the choice in the data directory', () async {
    final Directory folder = await Directory.systemTemp.createTemp('ui-lang');
    addTearDown(() => folder.delete(recursive: true));
    final FileAppLanguageStore store =
        FileAppLanguageStore(directory: () async => folder);
    expect(await store.read(), isNull);
    await store.write('ja');
    expect(await store.read(), 'ja');
    await store.write(null);
    expect(await store.read(), isNull);
  });

  testWidgets('choosing a language in Settings changes the text on screen',
      (WidgetTester tester) async {
    final _MemoryStore store = _MemoryStore();
    final AppLanguageController language = AppLanguageController(store: store);
    await language.load();
    final LogSettingsController logs = LogSettingsController(
      logBridge: FakeLogBridge(),
      exportSaver: FakeDesktopSaver(),
      sharedDirectory: () async => Directory.systemTemp,
    );
    await logs.load();
    await tester.pumpWidget(ListenableBuilder(
      listenable: language,
      builder: (BuildContext context, Widget? child) => MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        locale: language.locale ?? const Locale('en'),
        builder: (BuildContext context, Widget? navigator) =>
            AppLocalizationScope(child: navigator!),
        home: SettingsScreen(
            logSettingsController: logs, appLanguageController: language),
      ),
    ));
    expect(find.text('Log level'), findsOneWidget);

    await tester.tap(find.byKey(appLanguageSelectorKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Italiano').last);
    await tester.pumpAndSettle();

    expect(store.stored, 'it');
    expect(find.text('Impostazioni'), findsOneWidget);
    expect(find.text('Log level'), findsNothing);

    await language.choose('ar');
    await tester.pumpAndSettle();
    expect(Directionality.of(tester.element(find.byType(SettingsScreen))),
        TextDirection.rtl);
  });

  testWidgets('a system language the app lacks falls back to English (#198)',
      (WidgetTester tester) async {
    tester.platformDispatcher.localesTestValue = const <Locale>[Locale('nl')];
    addTearDown(tester.platformDispatcher.clearLocalesTestValue);
    late Locale shown;
    await tester.pumpWidget(MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Builder(builder: (BuildContext context) {
        shown = Localizations.localeOf(context);
        return const SizedBox();
      }),
    ));
    await tester.pumpAndSettle();
    expect(shown.languageCode, 'en');
  });
}
