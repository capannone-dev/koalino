// Coordinates attachment import state while decoding Rust-owned JSON only in Flutter.

import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:note_taker_ui/infrastructure/attachment_bridge.dart';

/// Supplies the production attachment workflow controller to Riverpod consumers.
final attachmentControllerProvider =
    ChangeNotifierProvider.autoDispose<AttachmentController>((Ref ref) {
  return AttachmentController(
    attachmentBridge: RustAttachmentBridge(),
    attachmentFilePicker: PlatformAttachmentFilePicker(),
  );
});

/// Renders one Rust-ranked meeting candidate without carrying a Rust struct over FFI.
final class AttachmentSuggestion {
  /// Creates immutable, presentation-safe meeting-link metadata.
  const AttachmentSuggestion({
    required this.attachmentId,
    required this.meetingId,
    required this.meetingStartTime,
    required this.meetingEndTime,
    required this.confidenceScore,
  });

  /// Opaque identifier retained by Rust for the selected import operation.
  final String attachmentId;

  /// Stable meeting identifier selected by the user.
  final String meetingId;

  /// UTC start time supplied for UI context.
  final DateTime meetingStartTime;

  /// UTC end time supplied for UI context.
  final DateTime meetingEndTime;

  /// Normalized Rust ranking score from zero through one.
  final double confidenceScore;

  /// Decodes one schema-validated suggestion object from the Rust JSON response.
  ///
  /// Inputs: `json` must use the documented snake-case C ABI response fields.
  /// Outputs: A displayable suggestion or `FormatException` for malformed content.
  /// Side effects: Parses ISO-8601 timestamps only; it does not call native code.
  factory AttachmentSuggestion.fromJson(Map<String, Object?> json) {
    final Object? attachmentId = json['attachment_id'];
    final Object? meetingId = json['meeting_id'];
    final Object? meetingStartTime = json['meeting_start_time'];
    final Object? meetingEndTime = json['meeting_end_time'];
    final Object? confidenceScore = json['confidence_score'];
    if (attachmentId is! String ||
        meetingId is! String ||
        meetingStartTime is! String ||
        meetingEndTime is! String ||
        confidenceScore is! num) {
      throw const FormatException('Attachment suggestion has invalid fields.');
    }
    return AttachmentSuggestion(
      attachmentId: attachmentId,
      meetingId: meetingId,
      meetingStartTime: DateTime.parse(meetingStartTime).toUtc(),
      meetingEndTime: DateTime.parse(meetingEndTime).toUtc(),
      confidenceScore: confidenceScore.toDouble(),
    );
  }
}

/// Owns picker, JSON-decoding, and confirmation state while widgets remain native-free.
final class AttachmentController extends ChangeNotifier {
  /// Creates a controller with independently injectable picker and Rust-boundary adapters.
  AttachmentController({
    required AttachmentBridge attachmentBridge,
    required AttachmentFilePicker attachmentFilePicker,
  })  : _attachmentBridge = attachmentBridge,
        _attachmentFilePicker = attachmentFilePicker;

  final AttachmentBridge _attachmentBridge;
  final AttachmentFilePicker _attachmentFilePicker;
  List<AttachmentSuggestion> _suggestions = const <AttachmentSuggestion>[];
  bool _isLoading = false;
  String? _errorMessage;

  /// Returns ranked suggestions decoded from the latest successful JSON response.
  List<AttachmentSuggestion> get suggestions => _suggestions;

  /// Reports whether picker or native work is in progress.
  bool get isLoading => _isLoading;

  /// Returns a display-safe import or link failure message.
  String? get errorMessage => _errorMessage;

  /// Opens the picker and decodes Rust JSON suggestions for one selected file.
  ///
  /// Inputs: None; cancellation is treated as a no-op.
  /// Outputs: The parsed candidate list or an empty list after cancellation/failure.
  /// Side effects: Opens a native picker, invokes Rust FFI, updates state, and notifies listeners.
  Future<List<AttachmentSuggestion>> pickAndLoadSuggestions() async {
    _isLoading = true;
    _errorMessage = null;
    _suggestions = const <AttachmentSuggestion>[];
    notifyListeners();
    try {
      final String? selectedFilePath =
          await _attachmentFilePicker.pickAttachmentPath();
      if (selectedFilePath == null) {
        return const <AttachmentSuggestion>[];
      }
      final String rawSuggestionsJson =
          await _attachmentBridge.processAttachment(selectedFilePath);
      _suggestions = _decodeSuggestions(rawSuggestionsJson);
      return _suggestions;
    } on Object {
      _errorMessage = 'Unable to import attachment';
      return const <AttachmentSuggestion>[];
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  /// Confirms one user-selected meeting association through the Rust boundary.
  ///
  /// Inputs: `suggestion` must originate from the current Rust JSON response.
  /// Outputs: Completes when Rust confirms the persistent link.
  /// Side effects: Invokes Rust FFI, clears imported candidates, and notifies listeners.
  Future<void> confirmSuggestion(AttachmentSuggestion suggestion) async {
    _isLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      await _attachmentBridge.confirmAttachmentLink(
        attachmentId: suggestion.attachmentId,
        meetingId: suggestion.meetingId,
      );
      _suggestions = const <AttachmentSuggestion>[];
    } on Object {
      _errorMessage = 'Unable to link attachment';
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  /// Validates and decodes the C ABI's JSON array in the Flutter controller boundary.
  ///
  /// Inputs: `rawSuggestionsJson` is the UTF-8 JSON text returned by Rust.
  /// Outputs: Immutable presentation values sorted by Rust's ranking order.
  /// Side effects: None; malformed native data throws `FormatException`.
  List<AttachmentSuggestion> _decodeSuggestions(String rawSuggestionsJson) {
    final Object? decodedJson = jsonDecode(rawSuggestionsJson);
    if (decodedJson is! List) {
      throw const FormatException('Attachment response must be a JSON array.');
    }
    return decodedJson.map<AttachmentSuggestion>((Object? value) {
      if (value is! Map) {
        throw const FormatException(
            'Attachment suggestion must be a JSON object.');
      }
      return AttachmentSuggestion.fromJson(value.cast<String, Object?>());
    }).toList(growable: false);
  }
}
