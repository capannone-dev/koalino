// Renders the recording lifecycle and optional meeting title using Material 3 controls, the
// live transcript growing chunk by chunk while recording (Issue #52), and the camera for photos
// of the meeting (Issue #54). A status panel sets recording apart from waiting, with the recorded
// time counting while recording (Issue #132).

import 'dart:async';

// ignore: deprecated_member_use
import 'package:note_taker_ui/controllers/transcription_language_controller.dart';
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/live_transcript_controller.dart';
import 'package:note_taker_ui/controllers/meeting_attachments_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

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
      ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.theRecordingIsStillStartingTry)));
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
      appBar: AppBar(title: const Text('Koalino')),
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
                      l10n.recording(title),
                    (RecordingUiState.recording, null) => l10n.recording2,
                    (RecordingUiState.paused, final String title) =>
                      l10n.paused(title),
                    (RecordingUiState.paused, null) => l10n.paused2,
                    (RecordingUiState.stopping, _) =>
                      l10n.finishingTranscription,
                    _ => l10n.readyToRecord,
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
                        decoration: InputDecoration(
                          labelText: l10n.meetingTitle,
                          hintText: l10n.enterMeetingTitleOptional,
                          helperText: l10n.leaveEmptyToNameTheMeeting,
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
                      _RecordingStatusPanel(
                        controller: recordingController,
                        state: state,
                        statusLabel: statusLabel,
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
                      if (recordingController.microphoneSilenced)
                        Padding(
                          padding: const EdgeInsets.only(top: 12),
                          child: Text(
                            l10n.microphoneSilencedByCall,
                            textAlign: TextAlign.center,
                            style: TextStyle(
                                color: Theme.of(context).colorScheme.error),
                          ),
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
                                  ? l10n.startingRecording
                                  : l10n.startRecording),
                            ),
                          if (state == RecordingUiState.recording)
                            FilledButton.tonalIcon(
                              onPressed: recordingController.pauseRecording,
                              icon: const Icon(Icons.pause),
                              label: Text(l10n.pause),
                            ),
                          if (state == RecordingUiState.paused)
                            FilledButton.icon(
                              onPressed: recordingController.resumeRecording,
                              icon: const Icon(Icons.play_arrow),
                              label: Text(l10n.resume),
                            ),
                          if (state == RecordingUiState.recording ||
                              state == RecordingUiState.paused ||
                              isStopping)
                            OutlinedButton.icon(
                              onPressed: isStopping ? null : _stopRecording,
                              icon: const Icon(Icons.stop),
                              label: Text(l10n.stop),
                            ),
                          if (activeMeetingId != null &&
                              photoScreenBuilder != null)
                            OutlinedButton.icon(
                              onPressed: () => _takePhotos(
                                  photoScreenBuilder, activeMeetingId),
                              icon: const Icon(Icons.photo_camera_outlined),
                              label: Text(l10n.takePhotos),
                            ),
                          if (activeMeetingId != null &&
                              attachmentsController != null)
                            OutlinedButton.icon(
                              onPressed: () => _attachFiles(
                                  attachmentsController, activeMeetingId),
                              icon: const Icon(Icons.attach_file),
                              label: Text(l10n.attachFiles),
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

/// Formats recorded time as `H:MM:SS`, e.g. `0:01:05` (#132).
///
/// Inputs: A non-negative duration. Outputs: The text. Side effects: None.
String formatRecordedTime(Duration recorded) {
  final int totalSeconds = recorded.isNegative ? 0 : recorded.inSeconds;
  final String minutes = (totalSeconds ~/ 60 % 60).toString().padLeft(2, '0');
  final String seconds = (totalSeconds % 60).toString().padLeft(2, '0');
  return '${totalSeconds ~/ 3600}:$minutes:$seconds';
}

/// Shows where the recorder stands, so recording and waiting can't be mistaken (#132).
///
/// Ready is a neutral panel with a microphone; recording is a red panel with a REC badge and the
/// recorded time counting every second; paused is a tertiary-colored panel whose time holds;
/// finishing shows the time it reached.
final class _RecordingStatusPanel extends StatefulWidget {
  const _RecordingStatusPanel({
    required this.controller,
    required this.state,
    required this.statusLabel,
  });

  final RecordingController controller;
  final RecordingUiState state;
  final String statusLabel;

  @override
  State<_RecordingStatusPanel> createState() => _RecordingStatusPanelState();
}

final class _RecordingStatusPanelState extends State<_RecordingStatusPanel> {
  /// Redraws the time every second while recording; `null` otherwise.
  Timer? _ticker;

  @override
  void initState() {
    super.initState();
    _followState();
  }

  @override
  void didUpdateWidget(covariant _RecordingStatusPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    _followState();
  }

  /// Ticks only while recording. Side effects: Starts or cancels the timer.
  void _followState() {
    final bool recording = widget.state == RecordingUiState.recording;
    if (recording && _ticker == null) {
      _ticker = Timer.periodic(const Duration(seconds: 1), (_) {
        if (mounted) {
          setState(() {});
        }
      });
    } else if (!recording) {
      _ticker?.cancel();
      _ticker = null;
    }
  }

  @override
  void dispose() {
    _ticker?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ColorScheme colors = Theme.of(context).colorScheme;
    final TextTheme text = Theme.of(context).textTheme;
    final RecordingUiState state = widget.state;
    final (Color background, Color foreground, IconData icon) = switch (state) {
      RecordingUiState.recording => (
          colors.errorContainer,
          colors.onErrorContainer,
          Icons.fiber_manual_record,
        ),
      RecordingUiState.paused => (
          colors.tertiaryContainer,
          colors.onTertiaryContainer,
          Icons.pause_circle_outline,
        ),
      RecordingUiState.stopping => (
          colors.secondaryContainer,
          colors.onSecondaryContainer,
          Icons.hourglass_top,
        ),
      RecordingUiState.idle || RecordingUiState.starting => (
          colors.surfaceContainerHighest,
          colors.onSurfaceVariant,
          Icons.mic_none,
        ),
    };
    final bool showsTime = state == RecordingUiState.recording ||
        state == RecordingUiState.paused ||
        state == RecordingUiState.stopping;
    return Card(
      key: ValueKey<String>('recording-status-${state.name}'),
      color: background,
      elevation: 0,
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 20),
        child: Column(
          children: <Widget>[
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: <Widget>[
                Icon(icon,
                    color: state == RecordingUiState.recording
                        ? colors.error
                        : foreground,
                    size: 28),
                if (state == RecordingUiState.recording) ...<Widget>[
                  const SizedBox(width: 8),
                  Container(
                    padding:
                        const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                    decoration: BoxDecoration(
                      color: colors.error,
                      borderRadius: BorderRadius.circular(4),
                    ),
                    child: Text(
                      'REC',
                      style: text.labelLarge?.copyWith(
                        color: colors.onError,
                        fontWeight: FontWeight.bold,
                        letterSpacing: 1.5,
                      ),
                    ),
                  ),
                ],
              ],
            ),
            if (showsTime)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  formatRecordedTime(widget.controller.recordedTime),
                  key: const ValueKey<String>('recording-timer'),
                  style: text.displayMedium?.copyWith(
                    color: foreground,
                    fontFeatures: const <FontFeature>[
                      FontFeature.tabularFigures()
                    ],
                  ),
                ),
              ),
            const SizedBox(height: 8),
            Text(
              widget.statusLabel,
              textAlign: TextAlign.center,
              style: text.titleMedium?.copyWith(color: foreground),
            ),
          ],
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
            labelText: l10n.language,
            helperText: isRecordingActive ? l10n.aChangeAppliesToWhatIs : null,
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
                Text(l10n.liveTranscript, style: theme.textTheme.titleMedium),
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
                              l10n.theTranscriptAppearsHereAFew,
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
