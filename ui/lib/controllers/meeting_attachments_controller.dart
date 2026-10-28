// Attaches several picked pictures or documents to a meeting at once (Issue #54). Rust copies
// them into the app's storage; while the meeting is being recorded they are placed at that
// moment of the transcript, otherwise at its end.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/meeting_attachment_bridge.dart';

/// Picks files and attaches them to a meeting.
final class MeetingAttachmentsController extends ChangeNotifier {
  /// Creates the controller over a picker and the Rust attachment bridge.
  MeetingAttachmentsController({
    required MeetingAttachmentBridge attachmentBridge,
    required AttachmentFilesPicker filesPicker,
  })  : _attachmentBridge = attachmentBridge,
        _filesPicker = filesPicker;

  final MeetingAttachmentBridge _attachmentBridge;
  final AttachmentFilesPicker _filesPicker;
  bool _isAttaching = false;

  /// Whether a pick or attach is in progress; the action is disabled meanwhile.
  bool get isAttaching => _isAttaching;

  /// Lets the owner pick files and attaches them to `meetingId`.
  ///
  /// Inputs: The meeting to attach to.
  /// Outputs: A message for a snack bar (`Attached 2 files`, with a count of files that could
  /// not be attached, or a failure), or `null` when the owner cancelled the picker.
  /// Side effects: Opens the picker; Rust copies the files and links them; notifies listeners.
  Future<String?> attachFiles(String meetingId) async {
    if (_isAttaching) {
      return null;
    }
    _isAttaching = true;
    notifyListeners();
    try {
      final List<String> filePaths = await _filesPicker.pickFiles();
      if (filePaths.isEmpty) {
        return null;
      }
      final AttachFilesResult result =
          await _attachmentBridge.attachFiles(meetingId, filePaths);
      final int attachedCount = result.attached.length;
      final String attached =
          'Attached $attachedCount ${attachedCount == 1 ? 'file' : 'files'}';
      return result.failedPaths.isEmpty
          ? attached
          : '$attached; ${result.failedPaths.length} could not be attached';
    } on Object {
      return 'The files could not be attached.';
    } finally {
      _isAttaching = false;
      notifyListeners();
    }
  }
}
