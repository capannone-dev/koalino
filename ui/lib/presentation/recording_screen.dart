// Renders the recording lifecycle and optional meeting title using Material 3 controls, the
// live transcript growing chunk by chunk while recording (Issue #52), and the camera for photos
// of the meeting (Issue #54).

// ignore: deprecated_member_use
import 'package:note_taker_ui/controllers/transcription_language_controller.dart';
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/live_transcript_controller.dart';
import 'package:note_taker_ui/controllers/meeting_attachments_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';

/// Longest title Rust accepts, in characters (`MeetingTitle::MAX_CHARACTERS`).
const int _maxMeetingTitleLength = 200;

/// Widest the title field and controls grow on desktop, in logical pixels.
const double _maxContentWidth = 480;

/// Tallest the live transcript grows before it scrolls, in logical pixels.
const double _maxTranscriptHeight = 320;

/// Finds the language selector in tests.
const Key recordingLanguageSelectorKey = Key('recording-language-selector');

/// Displays recording state and an optional title field, delegating actions to a controller.
final class RecordingScreen extends StatefulWidget {
  /// Creates a screen that observes `recordingController` for its displayable state.
  ///
  /// Inputs: `recordingController` must outlive this widget or be disposed by its owner;
  /// `languageController`, when given, adds the language selector, usable before and during a
  /// recording; `liveTranscriptController`, when given, adds the live transcript below the controls;
  /// `photoScreenBuilder` and `attachmentsController`, each with `activeMeetingId` (which reads
  /// the recorded meeting from Rust), add Take photos and Attach files while recording or
  /// paused; what they add is placed at that point of the transcript.
  /// Outputs: A Material 3 recording screen.
  /// Side effects: Registers an animated listener while the widget is mounted.
  const RecordingScreen({
    required this.recordingController,
    this.languageController,
    this.liveTranscriptController,
    this.photoScreenBuilder,
    this.attachmentsController,
    this.activeMeetingId,
    super.key,
  });

  final RecordingController recordingController;
  final TranscriptionLanguageController? languageController;
  final LiveTranscriptController? liveTranscriptController;
  final MeetingScreenBuilder? photoScreenBuilder;
  final MeetingAttachmentsController? attachmentsController;
  final Future<String?> Function()? activeMeetingId;

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

  /// Reads the meeting being recorded from Rust.
  ///
  /// Outputs: The meeting ID, or `null` after showing a snack bar when Rust does not report it
  /// yet (or the screen was closed meanwhile).
  Future<String?> _recordedMeetingId(
      Future<String?> Function() activeMeetingId) async {
    final String? meetingId = await activeMeetingId();
    if (!mounted) {
      return null;
    }
    if (meetingId == null) {
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
          content: Text('The recording is still starting; try again.')));
    }
    return meetingId;
  }

  /// Opens the camera for the meeting being recorded, so photos land at this point of its
  /// transcript.
  ///
  /// Side effects: Reads the recorded meeting from Rust and pushes the camera screen.
  Future<void> _takePhotos(MeetingScreenBuilder photoScreenBuilder,
      Future<String?> Function() activeMeetingId) async {
    final String? meetingId = await _recordedMeetingId(activeMeetingId);
    if (meetingId == null || !mounted) {
      return;
    }
    await Navigator.of(context).push(MaterialPageRoute<void>(
        builder: (BuildContext context) =>
            photoScreenBuilder(context, meetingId)));
  }

  /// Attaches picked files to the meeting being recorded, at this point of its transcript.
  ///
  /// Side effects: Reads the recorded meeting from Rust, opens the picker, attaches the files
  /// and shows the result in a snack bar.
  Future<void> _attachFiles(MeetingAttachmentsController attachmentsController,
      Future<String?> Function() activeMeetingId) async {
    final String? meetingId = await _recordedMeetingId(activeMeetingId);
    if (meetingId == null || !mounted) {
      return;
    }
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final String? message = await attachmentsController.attachFiles(meetingId);
    if (message != null) {
      messenger.showSnackBar(SnackBar(content: Text(message)));
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
                  final MeetingScreenBuilder? photoScreenBuilder =
                      widget.photoScreenBuilder;
                  final MeetingAttachmentsController? attachmentsController =
                      widget.attachmentsController;
                  // Photos and files can be added only while a meeting is being recorded.
                  final Future<String?> Function()? activeMeetingId =
                      state == RecordingUiState.recording ||
                              state == RecordingUiState.paused
                          ? widget.activeMeetingId
                          : null;
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
                      if (widget.languageController
                          case final TranscriptionLanguageController
                              languageController) ...<Widget>[
                        const SizedBox(height: 8),
                        _LanguageSelector(
                          controller: languageController,
                          isRecordingActive: isBusy,
                        ),
                      ],
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
                          if (activeMeetingId != null &&
                              photoScreenBuilder != null)
                            OutlinedButton.icon(
                              onPressed: () => _takePhotos(
                                  photoScreenBuilder, activeMeetingId),
                              icon: const Icon(Icons.photo_camera_outlined),
                              label: const Text('Take photos'),
                            ),
                          if (activeMeetingId != null &&
                              attachmentsController != null)
                            OutlinedButton.icon(
                              onPressed: () => _attachFiles(
                                  attachmentsController, activeMeetingId),
                              icon: const Icon(Icons.attach_file),
                              label: const Text('Attach files'),
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

/// Chooses the recording's language, before it starts and while it runs.
final class _LanguageSelector extends StatelessWidget {
  const _LanguageSelector({
    required this.controller,
    required this.isRecordingActive,
  });

  final TranscriptionLanguageController controller;

  /// Whether a recording is running, when a change applies to what is said next.
  final bool isRecordingActive;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: controller,
        builder: (BuildContext context, Widget? child) => InputDecorator(
          decoration: InputDecoration(
            labelText: 'Language',
            helperText: isRecordingActive
                ? 'A change applies to what is said next'
                : null,
            errorText: controller.message,
            prefixIcon: const Icon(Icons.translate),
            border: const OutlineInputBorder(),
          ),
          child: DropdownButtonHideUnderline(
            child: DropdownButton<String>(
              key: recordingLanguageSelectorKey,
              value: controller.recordingLanguage,
              isDense: true,
              isExpanded: true,
              menuMaxHeight: 400,
              onChanged: controller.isSaving
                  ? null
                  : (String? chosen) {
                      if (chosen != null) {
                        controller.chooseForRecording(chosen);
                      }
                    },
              items: <DropdownMenuItem<String>>[
                for (final LanguageChoice choice in controller.choices)
                  DropdownMenuItem<String>(
                      value: choice.code, child: Text(choice.label)),
              ],
            ),
          ),
        ),
      );
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
            // A line with no speaker information gets no label.
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
