// Owns the owner's edits to meetings in the app (#96): each meeting's notes as last read or saved,
// and corrections to single transcript segments, which are shown in the open meeting at once
// instead of reloading it. The core does the work; this keeps the answers and words failures.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_edit_bridge.dart';

/// Meeting notes and transcript corrections.
final class MeetingEditController extends ChangeNotifier {
  /// Creates the controller. Side effects: None until a method is called.
  MeetingEditController({MeetingEditBridge bridge = const MeetingEditBridge()})
      : _bridge = bridge;

  final MeetingEditBridge _bridge;

  /// Notes per meeting as last read or saved; a missing key means not loaded yet, an empty
  /// string means no notes.
  final Map<String, String> _notes = <String, String>{};

  /// Why the notes of a meeting could not be read, by meeting.
  final Map<String, String> _loadErrors = <String, String>{};
  bool _disposed = false;

  /// The meeting's notes, `''` when it has none, or `null` before they load.
  String? notesFor(String meetingId) => _notes[meetingId];

  /// Why the meeting's notes could not be read, or `null`.
  String? loadErrorFor(String meetingId) => _loadErrors[meetingId];

  /// Reads the meeting's notes.
  ///
  /// Side effects: Reads the meeting document through the core and notifies listeners.
  Future<void> loadNotes(String meetingId) async {
    try {
      _notes[meetingId] = await _bridge.notes(meetingId) ?? '';
      _loadErrors.remove(meetingId);
    } on MeetingEditException catch (error) {
      _loadErrors[meetingId] = error.message;
    }
    _notify();
  }

  /// Replaces the meeting's notes.
  ///
  /// Outputs: `null` once saved, or why not (the notes shown stay as they were).
  /// Side effects: Writes the meeting document, which syncs to paired devices.
  Future<String?> saveNotes(String meetingId, String text) async {
    try {
      _notes[meetingId] = await _bridge.setNotes(meetingId, text);
      _loadErrors.remove(meetingId);
      _notify();
      return null;
    } on MeetingEditException catch (error) {
      return error.message;
    }
  }

  /// Corrects one segment of the open meeting, or removes it when `text` is blank.
  ///
  /// Inputs: The shared meeting controller (its open detail is updated), the meeting, the segment
  /// as shown and the new text. Outputs: `null` once saved, or why not.
  /// Side effects: Flags the transcript as edited, so the after-meeting passes no longer replace
  /// it, and writes the meeting's segments, which sync to paired devices.
  Future<String?> editSegment(MeetingListController meetings, String meetingId,
      MeetingTranscriptSegment segment, String text) async {
    try {
      final String? stored = await _bridge.editSegment(
          meetingId, segment.startTimeMs, segment.endTimeMs, text);
      meetings.replaceSegment(meetingId, segment, stored);
      return null;
    } on MeetingEditException catch (error) {
      return error.message;
    }
  }

  void _notify() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
