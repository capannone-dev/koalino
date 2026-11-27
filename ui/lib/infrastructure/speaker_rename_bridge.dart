// Renames a speaker from a meeting's transcript (#186): which voice a transcript segment belongs
// to, and the calls that name it. A saved person is renamed everywhere through
// `note_taker_rename_person`; an unknown voice is named through `note_taker_assign_cluster`, as
// in Voices, which joins an existing person of the same name.
import 'dart:convert';

import 'voice_gallery_bridge.dart';

/// What kind of voice a transcript segment belongs to.
enum SpeakerVoiceKind {
  /// A person saved on this device, renamed everywhere.
  person,

  /// A voice nobody has named yet (an unknown-speaker cluster).
  unknown,
}

/// The voice of one transcript segment.
final class SpeakerVoice {
  /// Creates a voice of `kind` with Rust's `id` (identity or cluster UUID).
  const SpeakerVoice({required this.kind, required this.id});

  /// Decodes a segment's `voice` field; `null`, a missing field or an unknown kind is none.
  static SpeakerVoice? fromJson(Object? json) {
    if (json is! Map) {
      return null;
    }
    final Object? id = json['id'];
    final SpeakerVoiceKind? kind = switch (json['kind']) {
      'person' => SpeakerVoiceKind.person,
      'unknown' => SpeakerVoiceKind.unknown,
      _ => null,
    };
    return kind == null || id is! String
        ? null
        : SpeakerVoice(kind: kind, id: id);
  }

  /// Whether this is a saved person or an unknown voice.
  final SpeakerVoiceKind kind;

  /// The identity's or cluster's UUID.
  final String id;

  @override
  bool operator ==(Object other) =>
      other is SpeakerVoice && other.kind == kind && other.id == id;

  @override
  int get hashCode => Object.hash(kind, id);

  @override
  String toString() => 'SpeakerVoice(${kind.name}, $id)';
}

/// Why a speaker could not be renamed: Rust's error code, such as `name_taken`.
final class SpeakerRenameException implements Exception {
  /// Creates an exception for `code`.
  const SpeakerRenameException(this.code);

  /// Rust's `error_code`: `invalid_name`, `name_taken`, `person_not_found`, ... or `unknown`.
  final String code;

  @override
  String toString() => 'SpeakerRenameException($code)';
}

/// Names speakers for the meeting screen.
abstract interface class SpeakerRenamer {
  /// The saved people's names, offered as suggestions.
  Future<List<String>> savedNames();

  /// Gives `voice` the name `name`.
  ///
  /// Outputs: The stored name. Throws `SpeakerRenameException` when Rust refuses it.
  /// Side effects: Renames the saved person, or names the unknown voice, in every meeting.
  Future<String> rename(SpeakerVoice voice, String name);
}

/// The app's renamer, backed by the Rust core.
final class RustSpeakerRenamer implements SpeakerRenamer {
  /// Creates a renamer calling the core on background isolates.
  const RustSpeakerRenamer();

  @override
  Future<List<String>> savedNames() async =>
      decodeVoiceLibrary(await voiceLibraryJson())
          .savedPeople
          .map((SavedPerson person) => person.displayName)
          .toList(growable: false);

  @override
  Future<String> rename(SpeakerVoice voice, String name) async {
    switch (voice.kind) {
      case SpeakerVoiceKind.person:
        return decodePersonRename(await renamePersonJson(voice.id, name));
      case SpeakerVoiceKind.unknown:
        try {
          return decodeClusterAssignment(
                  await assignClusterJson(voice.id, name))
              .identityName;
        } on StateError catch (error) {
          throw SpeakerRenameException(_codeIn(error.message));
        }
    }
  }
}

/// Decodes `note_taker_rename_person`'s answer into the stored name.
///
/// Throws `SpeakerRenameException` with Rust's error code for `{"ok":false,...}`, and
/// `FormatException` for any other shape.
String decodePersonRename(String responseJson) {
  final Object? decoded = jsonDecode(responseJson);
  if (decoded is! Map) {
    throw const FormatException('Rename response must be a JSON object.');
  }
  if (decoded['ok'] != true) {
    final Object? code = decoded['error_code'];
    throw SpeakerRenameException(code is String ? code : 'unknown');
  }
  return decoded['identity_name']! as String;
}

/// Picks the error code out of the Voices bridge's `StateError` message, `(code): message`.
String _codeIn(String message) =>
    RegExp(r'\(([a-z_]+)\)').firstMatch(message)?.group(1) ?? 'unknown';
