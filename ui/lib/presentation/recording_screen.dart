// Renders the recording lifecycle and optional meeting title using Material 3 controls, and the
// live transcript growing chunk by chunk while recording (Issue #52).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/live_transcript_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';

/// Longest title Rust accepts, in characters (`MeetingTitle::MAX_CHARACTERS`).
const int _maxMeetingTitleLength = 200;

/// Widest the title field and controls grow on desktop, in logical pixels.
const double _maxContentWidth = 480;

/// Tallest the live transcript grows before it scrolls, in logical pixels.
const double _maxTranscriptHeight = 320;

/// Displays recording state and an optional title field, delegating actions to a controller.
final class RecordingScreen extends StatefulWidget {
  /// Creates a screen that observes `recordingController` for its displayable state.
  ///
  /// Inputs: `recordingController` must outlive this widget or be disposed by its owner;
  /// `liveTranscriptController`, when given, adds the live transcript below the controls.
  /// Outputs: A Material 3 recording screen.
  /// Side effects: Registers an animated listener while the widget is mounted.
  const RecordingScreen({
    required this.recordingController,
    this.liveTranscriptController,
    super.key,
  });

  final RecordingController recordingController;
  final LiveTranscriptController? liveTranscriptController;

  @override
  State<RecordingScreen> createState() => _RecordingScreenState();
}

/// Owns the title text field's editing state.
final class _RecordingScreenState extends State<RecordingScreen> {
  /// Holds the typed meeting title until a recording starts.
  final TextEditingController _titleController = TextEditingController();

  @override
  void dispose() {
    _titleController.dispose();
    super.dispose();
  }

  /// Starts a recording with the typed title (blank titles are sent as untitled).
  ///
  /// Inputs: None; reads the title field.
  /// Outputs: None.
  /// Side effects: Requests a Rust-owned recording through the controller.
  void _startRecording() {
    widget.recordingController.startRecording(title: _titleController.text);
  }

  /// Stops the recording and, once it is saved, clears the title for the next meeting.
  ///
  /// Inputs: None.
  /// Outputs: Completes after Rust finished transcribing.
  /// Side effects: Requests the stop through the controller; clears the title field on success.
  Future<void> _stopRecording() async {
    final RecordingController recordingController = widget.recordingController;
    await recordingController.stopRecording();
    if (mounted && recordingController.completionMessage != null) {
      _titleController.clear();
    }
  }

  /// Builds the title field, status label, messages, and the lifecycle controls.
  ///
  /// Inputs: `context` supplies the Material theme and localization context.
  /// Outputs: A widget tree representing the current controller state; it scrolls when the
  /// on-screen keyboard leaves too little room and is width-capped on large screens.
  /// Side effects: User interaction may request a Rust-owned recording transition through the
  /// controller.
  @override
  Widget build(BuildContext context) {
    final RecordingController recordingController = widget.recordingController;
    return Scaffold(
      appBar: AppBar(title: const Text('Note Taker')),
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(24),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: _maxContentWidth),
              child: AnimatedBuilder(
                animation: recordingController,
                builder: (BuildContext context, Widget? child) {
                  final RecordingUiState state = recordingController.state;
                  final bool isStarting = state == RecordingUiState.starting;
                  final bool isStopping = state == RecordingUiState.stopping;
                  final bool isBusy = state != RecordingUiState.idle;
                  final String? activeTitle = recordingController.activeTitle;
                  final String statusLabel = switch ((state, activeTitle)) {
                    (RecordingUiState.recording, final String title) =>
                      'Recording "$title"',
                    (RecordingUiState.recording, null) => 'Recording',
                    (RecordingUiState.paused, final String title) =>
                      'Paused "$title"',
                    (RecordingUiState.paused, null) => 'Paused',
                    (RecordingUiState.stopping, _) =>
                      'Finishing transcription…',
                    _ => 'Ready to record',
                  };

                  return Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      TextField(
                        controller: _titleController,
                        enabled: !isBusy,
                        maxLength: _maxMeetingTitleLength,
                        textCapitalization: TextCapitalization.sentences,
                        textInputAction: TextInputAction.done,
                        onSubmitted: (_) => _startRecording(),
                        decoration: const InputDecoration(
                          labelText: 'Meeting title',
                          hintText: 'Enter meeting title (optional)',
                          helperText:
                              'Leave empty to name the meeting by its start time',
                          prefixIcon: Icon(Icons.title),
                          border: OutlineInputBorder(),
                        ),
                      ),
                      const SizedBox(height: 16),
                      Text(
                        statusLabel,
                        textAlign: TextAlign.center,
                        style: Theme.of(context).textTheme.headlineSmall,
                      ),
                      if (isStopping)
                        const Padding(
                          padding: EdgeInsets.only(top: 12),
                          child: LinearProgressIndicator(),
                        ),
                      if (recordingController.completionMessage
                          case final String completionMessage)
                        Padding(
                          padding: const EdgeInsets.only(top: 12),
                          child: Text(completionMessage,
                              textAlign: TextAlign.center),
                        ),
                      if (recordingController.errorMessage
                          case final String errorMessage)
                        Padding(
                          padding: const EdgeInsets.only(top: 12),
                          child: Text(
                            errorMessage,
                            textAlign: TextAlign.center,
                            style: TextStyle(
                                color: Theme.of(context).colorScheme.error),
                          ),
                        ),
                      const SizedBox(height: 24),
                      Wrap(
                        alignment: WrapAlignment.center,
                        spacing: 12,
                        runSpacing: 12,
                        children: <Widget>[
                          if (state == RecordingUiState.idle || isStarting)
                            FilledButton.icon(
                              onPressed: isBusy ? null : _startRecording,
                              icon: const Icon(Icons.fiber_manual_record),
                              label: Text(isStarting
                                  ? 'Starting recording'
                                  : 'Start recording'),
                            ),
                          if (state == RecordingUiState.recording)
                            FilledButton.tonalIcon(
                              onPressed: recordingController.pauseRecording,
                              icon: const Icon(Icons.pause),
                              label: const Text('Pause'),
                            ),
                          if (state == RecordingUiState.paused)
                            FilledButton.icon(
                              onPressed: recordingController.resumeRecording,
                              icon: const Icon(Icons.play_arrow),
                              label: const Text('Resume'),
                            ),
                          if (state == RecordingUiState.recording ||
                              state == RecordingUiState.paused ||
                              isStopping)
                            OutlinedButton.icon(
                              onPressed: isStopping ? null : _stopRecording,
                              icon: const Icon(Icons.stop),
                              label: const Text('Stop'),
                            ),
                        ],
                      ),
                      if (widget.liveTranscriptController
                          case final LiveTranscriptController liveTranscript)
                        _LiveTranscript(controller: liveTranscript),
                    ],
                  );
                },
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The transcript of the meeting being recorded, newest line kept in view.
final class _LiveTranscript extends StatefulWidget {
  const _LiveTranscript({required this.controller});

  final LiveTranscriptController controller;

  @override
  State<_LiveTranscript> createState() => _LiveTranscriptState();
}

/// Scrolls to the newest line whenever one arrives.
final class _LiveTranscriptState extends State<_LiveTranscript> {
  final ScrollController _scrollController = ScrollController();
  int _shownSegmentCount = 0;

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  /// Scrolls to the end after the frame that added lines.
  void _followNewestLine(int segmentCount) {
    if (segmentCount == _shownSegmentCount) {
      return;
    }
    _shownSegmentCount = segmentCount;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.jumpTo(_scrollController.position.maxScrollExtent);
      }
    });
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: widget.controller,
        builder: (BuildContext context, Widget? child) {
          final LiveTranscriptController controller = widget.controller;
          if (!controller.isVisible) {
            return const SizedBox.shrink();
          }
          final List<MeetingTranscriptSegment> segments = controller.segments;
          _followNewestLine(segments.length);
          final ThemeData theme = Theme.of(context);
          return Padding(
            padding: const EdgeInsets.only(top: 32),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text('Live transcript', style: theme.textTheme.titleMedium),
                const SizedBox(height: 8),
                DecoratedBox(
                  decoration: BoxDecoration(
                    border: Border.all(color: theme.colorScheme.outlineVariant),
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: ConstrainedBox(
                    constraints:
                        const BoxConstraints(maxHeight: _maxTranscriptHeight),
                    child: segments.isEmpty
                        ? Padding(
                            padding: const EdgeInsets.all(16),
                            child: Text(
                              'The transcript appears here a few seconds after '
                              'each pause in speech.',
                              style: theme.textTheme.bodyMedium?.copyWith(
                                  color: theme.colorScheme.onSurfaceVariant),
                            ),
                          )
                        : ListView.separated(
                            controller: _scrollController,
                            shrinkWrap: true,
                            padding: const EdgeInsets.all(16),
                            itemCount: segments.length,
                            separatorBuilder: (BuildContext context, int _) =>
                                const SizedBox(height: 12),
                            itemBuilder: (BuildContext context, int index) =>
                                _TranscriptLine(segment: segments[index]),
                          ),
                  ),
                ),
              ],
            ),
          );
        },
      );
}

/// One line: its time, its speaker when one is known, and its text.
final class _TranscriptLine extends StatelessWidget {
  const _TranscriptLine({required this.segment});

  final MeetingTranscriptSegment segment;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final TextStyle? labelStyle = theme.textTheme.labelMedium
        ?.copyWith(color: theme.colorScheme.onSurfaceVariant);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Row(
          children: <Widget>[
            Text(formatMeetingDuration(segment.startTimeMs), style: labelStyle),
            // Until diarization exists (#44) every line is Unattributed; the label adds nothing.
            if (segment.speaker != 'Unattributed') ...<Widget>[
              const SizedBox(width: 8),
              Text(segment.speaker, style: labelStyle),
            ],
          ],
        ),
        const SizedBox(height: 2),
        Text(segment.text, style: theme.textTheme.bodyLarge),
      ],
    );
  }
}
