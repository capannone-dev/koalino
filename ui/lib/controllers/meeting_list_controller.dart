// Coordinates meeting list and meeting detail state exclusively through the meeting bridge.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';

/// Owns the meeting list, the selected meeting, and their loading and error states.
///
/// The list and the detail share one controller so the desktop two-pane layout and the phone
/// detail route render the same selection.
final class MeetingListController extends ChangeNotifier {
  /// Creates a controller whose native reads are delegated to `meetingBridge`.
  ///
  /// Inputs: `meetingBridge` implements the Dart meeting bridge contract.
  /// Outputs: A controller with no meetings loaded yet.
  /// Side effects: None.
  MeetingListController({required MeetingBridge meetingBridge})
      : _meetingBridge = meetingBridge;

  final MeetingBridge _meetingBridge;
  List<MeetingSummary> _meetings = const <MeetingSummary>[];
  bool _isLoading = false;
  String? _errorMessage;
  String? _selectedMeetingId;
  MeetingDetail? _selectedMeeting;
  bool _isLoadingDetail = false;
  String? _detailErrorMessage;

  /// Meetings in creation order, newest last, as Rust lists them.
  List<MeetingSummary> get meetings => _meetings;

  /// Whether the meeting list is being loaded.
  bool get isLoading => _isLoading;

  /// A displayable error for the latest failed list load, or `null`.
  String? get errorMessage => _errorMessage;

  /// The meeting whose detail is shown, or `null` before a selection.
  String? get selectedMeetingId => _selectedMeetingId;

  /// The loaded detail of the selected meeting, or `null` while loading or after a failure.
  MeetingDetail? get selectedMeeting => _selectedMeeting;

  /// Whether the selected meeting's detail is being loaded.
  bool get isLoadingDetail => _isLoadingDetail;

  /// A displayable error for the latest failed detail load, or `null`.
  String? get detailErrorMessage => _detailErrorMessage;

  /// Loads (or reloads) every local meeting.
  ///
  /// Inputs: None.
  /// Outputs: Completes once the list or a displayable error is available.
  /// Side effects: Reads SQLite through the bridge and notifies listeners. Rust error details
  /// are not shown, because they can contain storage paths.
  Future<void> loadMeetings() async {
    _isLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      _meetings = await _meetingBridge.getMeetings();
    } on Object {
      _errorMessage = 'Unable to load meetings';
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  /// Selects `meetingId` and loads its title, metadata, and transcript.
  ///
  /// Inputs: A meeting ID from the list (or any ID; unknown meetings report an error).
  /// Outputs: Completes once the detail or a displayable error is available. A response for an
  /// earlier selection that arrives after a newer one is discarded.
  /// Side effects: Reads SQLite through the bridge and notifies listeners.
  Future<void> selectMeeting(String meetingId) async {
    _selectedMeetingId = meetingId;
    _selectedMeeting = null;
    _isLoadingDetail = true;
    _detailErrorMessage = null;
    notifyListeners();
    MeetingDetail? loadedMeeting;
    String? loadError;
    try {
      loadedMeeting = await _meetingBridge.getMeeting(meetingId);
    } on Object {
      loadError = 'Unable to load this meeting';
    }
    if (_selectedMeetingId != meetingId) {
      return;
    }
    _selectedMeeting = loadedMeeting;
    _detailErrorMessage = loadError;
    _isLoadingDetail = false;
    notifyListeners();
  }

  /// Renames `meetingId` and updates the list row and open detail without reloading them.
  ///
  /// Inputs: The meeting ID and the title as typed; it is trimmed before it is sent. Callers
  /// validate it first with `validateMeetingTitle`, so Rust only rejects races and storage errors.
  /// Outputs: Completes on success; rethrows `MeetingRenameException` (or any bridge error) and
  /// leaves the titles unchanged on failure.
  /// Side effects: Writes the title through the bridge (CRDT document and SQLite) and notifies
  /// listeners. When the meeting no longer exists, reloads the list so the row disappears.
  Future<void> renameMeeting(String meetingId, String newTitle) async {
    final String trimmedTitle = newTitle.trim();
    try {
      await _meetingBridge.renameMeeting(meetingId, trimmedTitle);
    } on MeetingRenameException catch (error) {
      if (error.failure == MeetingRenameFailure.meetingNotFound) {
        await loadMeetings();
      }
      rethrow;
    }
    _meetings = <MeetingSummary>[
      for (final MeetingSummary meeting in _meetings)
        meeting.meetingId == meetingId
            ? _retitled(meeting, trimmedTitle)
            : meeting,
    ];
    if (_selectedMeeting case final MeetingDetail selectedMeeting
        when selectedMeeting.summary.meetingId == meetingId) {
      _selectedMeeting = MeetingDetail(
        summary: _retitled(selectedMeeting.summary, trimmedTitle),
        transcript: selectedMeeting.transcript,
        segments: selectedMeeting.segments,
      );
    }
    notifyListeners();
  }

  /// Deletes `meetingId` for good and removes it from the list and any open detail.
  ///
  /// Outputs: `true` when it is gone; `false` when Rust refused, leaving everything as it was.
  /// Side effects: Deletes the meeting through the bridge and notifies listeners.
  Future<bool> deleteMeeting(String meetingId) async {
    try {
      await _meetingBridge.deleteMeeting(meetingId);
    } on Object {
      return false;
    }
    _meetings = _meetings
        .where((MeetingSummary meeting) => meeting.meetingId != meetingId)
        .toList(growable: false);
    if (_selectedMeetingId == meetingId) {
      _selectedMeetingId = null;
      _selectedMeeting = null;
      _isLoadingDetail = false;
      _detailErrorMessage = null;
    }
    notifyListeners();
    return true;
  }
}

/// Returns a copy of `meeting` titled `title`, keeping its ID, creation time, and duration.
MeetingSummary _retitled(MeetingSummary meeting, String title) =>
    MeetingSummary(
      meetingId: meeting.meetingId,
      title: title,
      createdAt: meeting.createdAt,
      durationMs: meeting.durationMs,
    );
