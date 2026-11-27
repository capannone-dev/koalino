// Calls Settings → Developer in the core (#187) through `note_taker_developer_settings`: the app
// variables an expert can override, saved in the data folder and applied at the next start. The
// core owns the list, validation and storage; this file only sends commands and decodes answers,
// on background isolates.
import 'dart:convert';

import 'package:note_taker_ui/infrastructure/llm_bridge.dart';

/// What values a developer setting takes.
enum DeveloperSettingKind {
  /// `true` or `false`.
  flag,

  /// A whole number between `min` and `max`.
  integer,

  /// One of `options`.
  choice,

  /// `auto` or a language code such as `it`.
  language,

  /// A libp2p address such as `/ip4/0.0.0.0/tcp/4001`.
  multiaddress,
}

/// One overridable app variable, as the core describes it.
final class DeveloperSetting {
  /// Creates a setting.
  const DeveloperSetting({
    required this.name,
    required this.kind,
    required this.description,
    required this.defaultValue,
    required this.saved,
    required this.inEffect,
    this.options = const <String>[],
    this.min,
    this.max,
  });

  /// The variable name, such as `NOTE_TAKER_VAD_ENABLED`.
  final String name;

  /// What values it takes.
  final DeveloperSettingKind kind;

  /// What it does, in English (from the core).
  final String description;

  /// What the app uses when nothing sets it.
  final String defaultValue;

  /// The value saved for the next start, or null.
  final String? saved;

  /// The value the running app uses, or null when nothing sets it.
  final String? inEffect;

  /// The accepted words of a `choice`.
  final List<String> options;

  /// The bounds of an `integer`.
  final int? min;
  final int? max;
}

/// The section's state.
final class DeveloperSettingsState {
  /// Creates a state.
  const DeveloperSettingsState({
    required this.unlocked,
    required this.restartNeeded,
    required this.settings,
  });

  /// Whether the owner unlocked the section.
  final bool unlocked;

  /// Whether the saved values differ from those the app started with.
  final bool restartNeeded;

  /// Every overridable variable.
  final List<DeveloperSetting> settings;
}

/// A refused or failed request, with the core's code and message.
final class DeveloperSettingsException implements Exception {
  /// Creates the exception.
  const DeveloperSettingsException(this.code, this.message);

  /// Such as `invalid_value`, `locked` or `storage_failed`.
  final String code;

  /// The core's English explanation, such as what values are accepted.
  final String message;

  @override
  String toString() => 'DeveloperSettingsException($code)';
}

/// Sends Settings → Developer commands.
abstract interface class DeveloperSettingsBridge {
  /// Loads the state.
  Future<DeveloperSettingsState> get();

  /// Shows the section. Side effects: Writes the data folder's file.
  Future<DeveloperSettingsState> unlock();

  /// Clears every value and hides the section. Side effects: Writes the file.
  Future<DeveloperSettingsState> lock();

  /// Clears every value. Side effects: Writes the file.
  Future<DeveloperSettingsState> reset();

  /// Saves `value` for `name`, or clears it when null. Throws `DeveloperSettingsException`
  /// with `invalid_value` and what is accepted. Side effects: Writes the file.
  Future<DeveloperSettingsState> set(String name, String? value);
}

/// The app's bridge, backed by the Rust core.
final class RustDeveloperSettingsBridge implements DeveloperSettingsBridge {
  /// Creates the bridge; `call` is replaced in tests.
  const RustDeveloperSettingsBridge({LlmNativeCall call = callRustLlm})
      : _call = call;

  final LlmNativeCall _call;

  Future<DeveloperSettingsState> _command(Map<String, Object?> request) async =>
      decodeDeveloperSettings(await _call(
          'note_taker_developer_settings', <String?>[jsonEncode(request)]));

  @override
  Future<DeveloperSettingsState> get() =>
      _command(<String, Object?>{'command': 'get'});

  @override
  Future<DeveloperSettingsState> unlock() =>
      _command(<String, Object?>{'command': 'unlock'});

  @override
  Future<DeveloperSettingsState> lock() =>
      _command(<String, Object?>{'command': 'lock'});

  @override
  Future<DeveloperSettingsState> reset() =>
      _command(<String, Object?>{'command': 'reset'});

  @override
  Future<DeveloperSettingsState> set(String name, String? value) =>
      _command(<String, Object?>{
        'command': 'set',
        'name': name,
        'value': value,
      });
}

/// Decodes `note_taker_developer_settings`' answer.
///
/// Throws `DeveloperSettingsException` for `{"ok":false,...}` and for an unreadable answer.
DeveloperSettingsState decodeDeveloperSettings(String answer) {
  final Object? decoded;
  try {
    decoded = jsonDecode(answer);
  } on FormatException {
    throw const DeveloperSettingsException('unreadable', 'unreadable answer');
  }
  if (decoded is! Map) {
    throw const DeveloperSettingsException('unreadable', 'unreadable answer');
  }
  if (decoded['ok'] != true) {
    final Object? code = decoded['error_code'];
    final Object? message = decoded['error'];
    throw DeveloperSettingsException(
      code is String ? code : 'unknown',
      message is String ? message : '',
    );
  }
  final List<DeveloperSetting> settings = <DeveloperSetting>[
    for (final Object? entry
        in decoded['settings'] as List<Object?>? ?? <Object?>[])
      if (entry is Map) _decodeSetting(entry),
  ];
  return DeveloperSettingsState(
    unlocked: decoded['unlocked'] == true,
    restartNeeded: decoded['restart_needed'] == true,
    settings: settings,
  );
}

DeveloperSetting _decodeSetting(Map<Object?, Object?> json) => DeveloperSetting(
      name: json['name']! as String,
      kind: switch (json['kind']) {
        'flag' => DeveloperSettingKind.flag,
        'integer' => DeveloperSettingKind.integer,
        'choice' => DeveloperSettingKind.choice,
        'language' => DeveloperSettingKind.language,
        _ => DeveloperSettingKind.multiaddress,
      },
      description: json['description'] as String? ?? '',
      defaultValue: json['default'] as String? ?? '',
      saved: json['saved'] as String?,
      inEffect: json['in_effect'] as String?,
      options: (json['options'] as List<Object?>? ?? <Object?>[])
          .whereType<String>()
          .toList(growable: false),
      min: (json['min'] as num?)?.toInt(),
      max: (json['max'] as num?)?.toInt(),
    );
