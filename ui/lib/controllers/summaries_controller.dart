// Owns the AI connections and meeting summaries state (#95), shared by Settings, the meeting
// lists and the summary page. The core does the work; this keeps the latest answers, tracks the
// summaries being written, and words failures for people. On a phone it also asks a paired
// computer to write a summary (#107); on a computer it chooses the connection that answers.

import 'package:note_taker_ui/controllers/handoff_controller.dart';
import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_edit_bridge.dart';

/// AI settings and each meeting's summary, as last read from the core.
final class SummariesController extends ChangeNotifier {
  /// Creates the controller; `editBridge` saves the owner's own summary text (#96).
  /// Side effects: None until a method is called.
  SummariesController({
    required LlmBridge bridge,
    MeetingEditBridge editBridge = const MeetingEditBridge(),
    this.handoff,
    this.asksPairedComputer = false,
  })  : _bridge = bridge,
        _editBridge = editBridge;

  final LlmBridge _bridge;
  final MeetingEditBridge _editBridge;

  /// Hands a meeting to the owner's own AI app (#114); null where that is not offered (desktops).
  final HandoffController? handoff;

  /// Whether this device asks a paired computer for summaries (phones), rather than answering
  /// paired devices itself (computers) (#107).
  final bool asksPairedComputer;
  LlmSettings? _settings;
  String? _message;
  bool _isBusy = false;
  bool _disposed = false;
  final Map<String, MeetingSummaryState> _meetings =
      <String, MeetingSummaryState>{};
  final Map<String, String> _meetingMessages = <String, String>{};
  final Set<String> _summarizing = <String>{};

  /// Bumped per meeting whenever its state is written, so a slower read never replaces a newer
  /// summary or prompt.
  final Map<String, int> _versions = <String, int>{};

  /// The settings, or `null` before they load or when the core can't be reached.
  LlmSettings? get settings => _settings;

  /// The saved connections, empty before the settings load.
  List<LlmConnection> get connections =>
      _settings?.connections ?? const <LlmConnection>[];

  /// The default connection, or `null` without one.
  LlmConnection? get defaultConnection {
    for (final LlmConnection connection in connections) {
      if (connection.isDefault) {
        return connection;
      }
    }
    return null;
  }

  /// The latest settings confirmation or failure, or `null`.
  String? get message => _message;

  /// Whether a settings change or a connection test is running.
  bool get isBusy => _isBusy;

  /// The meeting's summary and prompt as last read, or `null` before it loads.
  MeetingSummaryState? stateFor(String meetingId) => _meetings[meetingId];

  /// The latest summary confirmation or failure for the meeting, or `null`.
  String? meetingMessage(String meetingId) => _meetingMessages[meetingId];

  /// Whether the meeting's summary is being written.
  bool isSummarizing(String meetingId) => _summarizing.contains(meetingId);

  /// Reads the settings.
  ///
  /// Side effects: Calls the core and notifies listeners; a failure leaves `settings` null and
  /// sets `message`.
  Future<void> load() async {
    try {
      _settings = await _bridge.settings();
    } on LlmException catch (error) {
      _settings = null;
      _message = error.message;
    }
    _notify();
  }

  /// Creates a connection, or edits the one with `id`, then reloads the settings.
  ///
  /// Outputs: Whether it was saved; `message` says why not. Side effects: Writes the settings
  /// file and the key store through the core.
  Future<bool> saveConnection(LlmConnectionInput input, {String? id}) =>
      _change(() async {
        final LlmConnection saved = await _bridge.saveConnection(input, id: id);
        _settings = await _bridge.settings();
        return 'Saved ${saved.label}.';
      });

  /// Removes a connection and its key. Outputs: Whether it was removed.
  Future<bool> deleteConnection(String id) => _change(() async {
        final String label = _labelOf(id);
        await _bridge.deleteConnection(id);
        _settings = await _bridge.settings();
        return 'Removed $label.';
      });

  /// Makes `id` the default connection. Outputs: Whether it was saved.
  Future<bool> makeDefault(String id) => _change(() async {
        _settings = await _bridge.update(defaultConnection: id);
        return '${_labelOf(id)} is now the default.';
      });

  /// Asks the connection a fixed one-line question, holding no meeting content.
  ///
  /// Side effects: One request to the connection's server; `message` reports the reply.
  Future<bool> testConnection(String id) => _change(() async {
        final String reply = await _bridge.testConnection(id);
        return '${_labelOf(id)} answered: $reply';
      });

  /// Saves the summary prompt template. Outputs: Whether it was accepted.
  Future<bool> saveTemplate(String template) => _change(() async {
        _settings = await _bridge.update(template: template);
        return 'Prompt saved.';
      });

  /// Restores the built-in prompt template.
  Future<bool> resetTemplate() => _change(() async {
        _settings = await _bridge.update(resetTemplate: true);
        return 'The built-in prompt is back.';
      });

  /// Turns automatic summaries of finished meetings on or off (#101).
  Future<bool> setAutoSummarize({required bool enabled}) => _change(() async {
        _settings = await _bridge.update(autoSummarize: enabled);
        return enabled
            ? 'Finished meetings will be summarized automatically.'
            : 'Automatic summaries are off.';
      });

  /// Chooses the connection that writes the summaries paired devices ask for, or stops this
  /// device answering them with null (#107). Outputs: Whether it was saved.
  Future<bool> setPairedSummaryConnection(String? id) => _change(() async {
        _settings = id == null
            ? await _bridge.update(stopAnsweringPairedDevices: true)
            : await _bridge.update(pairedSummaryConnection: id);
        return id == null
            ? 'This computer no longer writes summaries for your other devices.'
            : '${_labelOf(id)} now writes the summaries your other devices ask for.';
      });

  /// Asks a paired computer to write the meeting's summary (#107); a failed request is replaced.
  ///
  /// Side effects: Writes the request into the meeting document, which syncs to the computer.
  /// Nothing is sent to an AI server from this device.
  Future<void> askComputer(String meetingId) =>
      _changeRequest(meetingId, cancel: false);

  /// Cancels the meeting's request to a paired computer (#107).
  ///
  /// Side effects: Removes the request from the meeting document. A computer already writing
  /// the summary may still store it.
  Future<void> cancelComputer(String meetingId) =>
      _changeRequest(meetingId, cancel: true);

  Future<void> _changeRequest(String meetingId, {required bool cancel}) async {
    try {
      final PairedSummaryRequest? request =
          await _bridge.requestPairedSummary(meetingId, cancel: cancel);
      _bump(meetingId);
      _meetings[meetingId] = MeetingSummaryState(
        summary: _meetings[meetingId]?.summary,
        meetingPrompt: _meetings[meetingId]?.meetingPrompt,
        request: request,
      );
      if (cancel) {
        _meetingMessages[meetingId] = 'Request cancelled.';
      } else {
        _meetingMessages.remove(meetingId);
      }
    } on LlmException catch (error) {
      _meetingMessages[meetingId] = error.message;
    }
    _notify();
  }

  /// Reads a meeting's summary and its own prompt.
  ///
  /// Side effects: Reads the meeting document through the core; a failure is kept as the
  /// meeting's message.
  Future<void> loadMeeting(String meetingId) async {
    final int version = _versions[meetingId] ?? 0;
    try {
      final MeetingSummaryState state = await _bridge.meetingSummary(meetingId);
      if ((_versions[meetingId] ?? 0) == version) {
        _meetings[meetingId] = state;
      }
    } on LlmException catch (error) {
      _meetingMessages[meetingId] = error.message;
    }
    _notify();
  }

  /// Writes a meeting's summary with `connectionId`, or the default connection when null.
  ///
  /// Side effects: Sends the meeting to the connection's server and stores the summary, which
  /// syncs to paired devices. A second call while one runs for the same meeting is ignored. A
  /// failure keeps the previous summary.
  Future<void> summarize(String meetingId, {String? connectionId}) async {
    if (!_summarizing.add(meetingId)) {
      return;
    }
    _meetingMessages.remove(meetingId);
    _notify();
    try {
      final SummaryResult result =
          await _bridge.summarize(meetingId, connectionId: connectionId);
      _bump(meetingId);
      _meetings[meetingId] = MeetingSummaryState(
        summary: result.summary,
        meetingPrompt: _meetings[meetingId]?.meetingPrompt,
        request: _meetings[meetingId]?.request,
      );
      _meetingMessages[meetingId] = describeSummaryResult(result);
    } on LlmException catch (error) {
      _meetingMessages[meetingId] = error.message;
    } finally {
      _summarizing.remove(meetingId);
      _notify();
    }
  }

  /// Sets the meeting's own prompt, or returns it to the Settings prompt when `prompt` is null.
  ///
  /// Outputs: Whether it was saved. Side effects: Writes the meeting document.
  Future<bool> setMeetingPrompt(String meetingId, String? prompt) async {
    bool saved = false;
    try {
      await _bridge.setMeetingPrompt(meetingId, prompt);
      _bump(meetingId);
      _meetings[meetingId] = MeetingSummaryState(
        summary: _meetings[meetingId]?.summary,
        meetingPrompt: prompt,
        request: _meetings[meetingId]?.request,
      );
      _meetingMessages[meetingId] = prompt == null
          ? 'This meeting uses the Settings prompt again.'
          : 'Prompt saved for this meeting.';
      saved = true;
    } on LlmException catch (error) {
      _meetingMessages[meetingId] = error.message;
    }
    _notify();
    return saved;
  }

  /// Replaces the summary's text with the owner's, or writes one by hand when there is none (#96).
  ///
  /// Outputs: `null` once saved, or why not. Side effects: Writes the meeting document; the
  /// summary is marked edited, so automatic summaries never replace it.
  Future<String?> editSummary(String meetingId, String text) async {
    try {
      final StoredSummary saved =
          await _editBridge.editSummary(meetingId, text);
      _bump(meetingId);
      _meetings[meetingId] = MeetingSummaryState(
        summary: saved,
        meetingPrompt: _meetings[meetingId]?.meetingPrompt,
        request: _meetings[meetingId]?.request,
      );
      _meetingMessages[meetingId] = 'Summary saved.';
      _notify();
      return null;
    } on MeetingEditException catch (error) {
      return error.message;
    }
  }

  /// Runs one settings change, keeping its confirmation or failure in `message`.
  Future<bool> _change(Future<String> Function() change) async {
    _isBusy = true;
    _message = null;
    _notify();
    bool done = false;
    try {
      _message = await change();
      done = true;
    } on LlmException catch (error) {
      _message = error.message;
    } finally {
      _isBusy = false;
      _notify();
    }
    return done;
  }

  /// Marks the meeting's state as newer than any read in flight.
  void _bump(String meetingId) =>
      _versions[meetingId] = (_versions[meetingId] ?? 0) + 1;

  /// The label of connection `id`, or a generic name.
  String _labelOf(String id) {
    for (final LlmConnection connection in connections) {
      if (connection.id == id) {
        return connection.label;
      }
    }
    return 'The connection';
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

/// Words a written summary's report: how many parts and pictures.
///
/// Inputs: The core's result. Outputs: One or more sentences. Side effects: None.
String describeSummaryResult(SummaryResult result) {
  final StringBuffer text = StringBuffer('Summary written.');
  if (result.parts > 1) {
    text.write(' The transcript was read in ${result.parts} parts.');
  }
  if (result.imagesSent > 0 || result.imagesSkipped > 0) {
    final String sent = result.imagesSent == 1
        ? '1 picture was included'
        : '${result.imagesSent} pictures were included';
    text.write(result.imagesSkipped == 0
        ? ' $sent.'
        : ' $sent, ${result.imagesSkipped} left out.');
  }
  if (result.summary.truncated) {
    text.write(' The model stopped before the end.');
  }
  return text.toString();
}

/// Words a request to a paired computer (#107) for people.
///
/// Inputs: The request. Outputs: One sentence; a failure uses the computer's error code, worded
/// as on the computer. Side effects: None.
String describePairedRequest(PairedSummaryRequest request) =>
    switch (request.state) {
      PairedRequestState.waiting =>
        'Waiting for your computer. It writes the summary once it is on, connected to this '
            'device, and set to answer in Settings → AI connections.',
      PairedRequestState.working => 'Your computer is writing the summary…',
      PairedRequestState.failed => 'Your computer could not write the summary: '
          '${LlmException(request.errorCode ?? 'unknown').message}',
    };
