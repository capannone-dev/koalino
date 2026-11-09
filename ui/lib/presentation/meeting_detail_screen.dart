// Renders one meeting with its title as the header, followed by metadata and transcript, with
// its photos and documents placed where they were taken (Issue #54).

import 'dart:io';

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent;
import 'package:note_taker_ui/controllers/meeting_edit_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/presentation/meeting_edit_widgets.dart';
import 'package:note_taker_ui/presentation/meeting_summary_screen.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/controllers/meeting_attachments_controller.dart';
import 'package:note_taker_ui/controllers/meeting_export_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/controllers/transcription_language_controller.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_timeline.dart';
import 'package:note_taker_ui/presentation/delete_meeting_action.dart';
import 'package:note_taker_ui/presentation/split_meeting_action.dart';
import 'package:note_taker_ui/presentation/rename_meeting_dialog.dart';

/// Builds a screen about one meeting, such as the voice gallery for its unknown speakers.
typedef MeetingScreenBuilder = Widget Function(
    BuildContext context, String meetingId);

/// Widest the detail content grows, in logical pixels, so transcript lines stay readable.
const double _maxReadableWidth = 840;

/// Shows one meeting as a full screen; used on phones, where the list pushes it as a route.
final class MeetingDetailScreen extends StatefulWidget {
  /// Creates a screen that selects and renders `meetingId` through `controller`.
  ///
  /// Inputs: The shared meeting controller, the meeting to show, the attachments controller
  /// (Attach files), optional builders for the camera (Take photos) and the meeting's speakers,
  /// and the export controller (each action is hidden without its builder or controller).
  /// Outputs: A Material 3 detail screen.
  /// Side effects: Loads the meeting through the controller after the first frame, unless the
  /// caller already selected it (the list does, so the route never shows another meeting).
  const MeetingDetailScreen({
    required this.controller,
    required this.meetingId,
    this.attachmentsController,
    this.photoScreenBuilder,
    this.speakersScreenBuilder,
    this.exportController,
    this.projectsController,
    this.summariesController,
    this.editController,
    this.focusStartMs,
    super.key,
  });

  final MeetingListController controller;
  final String meetingId;

  /// Scrolls to and marks the transcript passage starting here, such as a search result (#100).
  final int? focusStartMs;
  final MeetingAttachmentsController? attachmentsController;
  final MeetingScreenBuilder? photoScreenBuilder;
  final MeetingScreenBuilder? speakersScreenBuilder;
  final MeetingExportController? exportController;

  /// Offers Move to project when given (Issue #85).
  final ProjectsController? projectsController;

  /// Adds the Summary button when given (#95).
  final SummariesController? summariesController;

  /// Shows the notes and lets the owner correct the transcript when given (#96).
  final MeetingEditController? editController;

  @override
  State<MeetingDetailScreen> createState() => _MeetingDetailScreenState();
}

/// Requests the meeting once, when the route is first shown without a matching selection.
final class _MeetingDetailScreenState extends State<MeetingDetailScreen> {
  @override
  void initState() {
    super.initState();
    if (widget.controller.selectedMeetingId != widget.meetingId) {
      // Selecting notifies listeners, which must not happen while this route is building.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          widget.controller.selectMeeting(widget.meetingId);
        }
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Meeting'),
        actions: <Widget>[
          if (widget.summariesController
              case final SummariesController summaries)
            ListenableBuilder(
              listenable: widget.controller,
              builder: (BuildContext context, Widget? child) =>
                  MeetingSummaryButton(
                controller: summaries,
                meetingId: widget.meetingId,
                title: widget.controller.selectedMeeting?.summary.title ??
                    'this meeting',
              ),
            ),
          if (widget.speakersScreenBuilder
              case final MeetingScreenBuilder builder)
            NameSpeakersButton(
              controller: widget.controller,
              meetingId: widget.meetingId,
              speakersScreenBuilder: builder,
            ),
          if (widget.exportController
              case final MeetingExportController exportController)
            ExportMeetingButton(
              meetingController: widget.controller,
              exportController: exportController,
              meetingId: widget.meetingId,
            ),
          if (widget.photoScreenBuilder case final MeetingScreenBuilder builder)
            TakePhotosButton(
              controller: widget.controller,
              meetingId: widget.meetingId,
              photoScreenBuilder: builder,
            ),
          if (widget.attachmentsController
              case final MeetingAttachmentsController attachmentsController)
            AttachFilesButton(
              controller: widget.controller,
              attachmentsController: attachmentsController,
              meetingId: widget.meetingId,
            ),
          if (widget.projectsController
              case final ProjectsController projectsController)
            MoveToProjectButton(
              controller: projectsController,
              meetingId: widget.meetingId,
            ),
        ],
      ),
      body: MeetingDetailView(
          controller: widget.controller,
          editController: widget.editController,
          focusStartMs: widget.focusStartMs),
    );
  }
}

/// Opens the multi-shot camera for a meeting from its app bar (Issue #54).
final class TakePhotosButton extends StatelessWidget {
  /// Creates the action for `meetingId`.
  ///
  /// Inputs: The meeting controller, the meeting, and the builder of the camera screen.
  /// Outputs: An icon button.
  /// Side effects: On return from the camera, reloads the meeting, so the new photos show.
  const TakePhotosButton({
    required this.controller,
    required this.meetingId,
    required this.photoScreenBuilder,
    super.key,
  });

  final MeetingListController controller;
  final String meetingId;
  final MeetingScreenBuilder photoScreenBuilder;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      tooltip: 'Take photos',
      icon: const Icon(Icons.photo_camera_outlined),
      onPressed: () async {
        await Navigator.of(context).push(MaterialPageRoute<void>(
            builder: (BuildContext context) =>
                photoScreenBuilder(context, meetingId)));
        await controller.selectMeeting(meetingId);
      },
    );
  }
}

/// Attaches several pictures or documents to a meeting from its app bar (Issue #54).
final class AttachFilesButton extends StatelessWidget {
  /// Creates the action for `meetingId`.
  ///
  /// Inputs: The meeting controller, the attachments controller, and the meeting.
  /// Outputs: An icon button, disabled while files are being attached.
  /// Side effects: Opens the picker, attaches the picked files, shows a snack bar with the
  /// result, and reloads the meeting.
  const AttachFilesButton({
    required this.controller,
    required this.attachmentsController,
    required this.meetingId,
    super.key,
  });

  final MeetingListController controller;
  final MeetingAttachmentsController attachmentsController;
  final String meetingId;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: attachmentsController,
      builder: (BuildContext context, Widget? child) => IconButton(
        tooltip: 'Attach files',
        icon: const Icon(Icons.attach_file),
        onPressed: attachmentsController.isAttaching
            ? null
            : () async {
                final ScaffoldMessengerState messenger =
                    ScaffoldMessenger.of(context);
                final String? message =
                    await attachmentsController.attachFiles(meetingId);
                if (message == null) {
                  return;
                }
                messenger.showSnackBar(SnackBar(content: Text(message)));
                await controller.selectMeeting(meetingId);
              },
      ),
    );
  }
}

/// Exports the meeting from its app bar: a menu of formats, then a snack bar with the result
/// (Issue #48).
final class ExportMeetingButton extends StatelessWidget {
  /// Creates the action for `meetingId`.
  ///
  /// Inputs: The meeting controller (for the displayed title), the export controller, and the
  /// meeting.
  /// Outputs: A menu button, disabled while an export runs.
  /// Side effects: Runs the export, which may show a save dialog, then shows a snack bar
  /// (nothing when the owner cancels the dialog).
  const ExportMeetingButton({
    required this.meetingController,
    required this.exportController,
    required this.meetingId,
    super.key,
  });

  final MeetingListController meetingController;
  final MeetingExportController exportController;
  final String meetingId;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: exportController,
      builder: (BuildContext context, Widget? child) =>
          PopupMenuButton<MeetingExportFormat>(
        tooltip: 'Export',
        enabled: !exportController.isExporting,
        icon: exportController.isExporting
            ? const SizedBox.square(
                dimension: 20, child: CircularProgressIndicator(strokeWidth: 2))
            : const Icon(Icons.file_download_outlined),
        onSelected: (MeetingExportFormat format) => _export(context, format),
        itemBuilder: (BuildContext context) =>
            <PopupMenuEntry<MeetingExportFormat>>[
          for (final MeetingExportFormat format in MeetingExportFormat.values)
            PopupMenuItem<MeetingExportFormat>(
                value: format, child: Text(format.label)),
        ],
      ),
    );
  }

  /// Runs the export and reports its result.
  Future<void> _export(BuildContext context, MeetingExportFormat format) async {
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final String title =
        meetingController.selectedMeeting?.summary.title ?? 'meeting';
    final String? message = await exportController.exportMeeting(
        meetingId: meetingId, title: title, format: format);
    if (message != null) {
      messenger.showSnackBar(SnackBar(content: Text(message)));
    }
  }
}

/// Opens the meeting's speakers (the voice gallery) from its app bar.
final class NameSpeakersButton extends StatelessWidget {
  /// Creates the action for `meetingId`.
  ///
  /// Inputs: The meeting controller, the meeting, and the builder of its speakers screen.
  /// Outputs: An icon button.
  /// Side effects: On return from the speakers screen, reloads the meeting through the
  /// controller, so the transcript shows the names just assigned.
  const NameSpeakersButton({
    required this.controller,
    required this.meetingId,
    required this.speakersScreenBuilder,
    super.key,
  });

  final MeetingListController controller;
  final String meetingId;
  final MeetingScreenBuilder speakersScreenBuilder;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      tooltip: 'Name speakers',
      icon: const Icon(Icons.record_voice_over_outlined),
      onPressed: () async {
        await Navigator.of(context).push(MaterialPageRoute<void>(
            builder: (BuildContext context) =>
                speakersScreenBuilder(context, meetingId)));
        await controller.selectMeeting(meetingId);
      },
    );
  }
}

/// Renders the controller's selected meeting; shared by the phone route and the desktop pane.
final class MeetingDetailView extends StatelessWidget {
  /// Creates a view observing `controller`'s selection; `editController` adds the notes and
  /// transcript corrections (#96).
  const MeetingDetailView(
      {required this.controller,
      this.editController,
      this.focusStartMs,
      super.key});

  final MeetingListController controller;
  final MeetingEditController? editController;

  /// Scrolls to and marks the transcript passage starting here (#100).
  final int? focusStartMs;

  /// Builds a loading indicator, an error, a selection prompt, or the meeting content.
  ///
  /// Inputs: `context` supplies the Material theme.
  /// Outputs: The widget tree for the current selection state.
  /// Side effects: None.
  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (BuildContext context, Widget? child) {
        if (controller.isLoadingDetail) {
          return const Center(child: CircularProgressIndicator());
        }
        if (controller.detailErrorMessage case final String errorMessage) {
          return Center(child: Text(errorMessage));
        }
        return switch (controller.selectedMeeting) {
          final MeetingDetail meeting => _MeetingContent(
              controller: controller,
              meeting: meeting,
              editController: editController,
              focusStartMs: focusStartMs),
          null => const Center(
              child: Text('Select a meeting to see its transcript.')),
        };
      },
    );
  }
}

/// Lays out the title header (with its rename action), metadata, and transcript of a meeting.
final class _MeetingContent extends StatelessWidget {
  const _MeetingContent(
      {required this.controller,
      required this.meeting,
      this.editController,
      this.focusStartMs});

  final MeetingListController controller;
  final MeetingDetail meeting;
  final MeetingEditController? editController;
  final int? focusStartMs;

  /// The start of the passage to scroll to: the one starting at `focusStartMs`, or the last one
  /// starting before it; null without a focus or segments.
  int? get _focusedStart {
    final int? focus = focusStartMs;
    if (focus == null) {
      return null;
    }
    int? best;
    for (final MeetingTranscriptSegment segment in meeting.segments) {
      if (segment.startTimeMs <= focus &&
          (best == null || segment.startTimeMs > best)) {
        best = segment.startTimeMs;
      }
    }
    return best;
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final MeetingSummary summary = meeting.summary;
    return Align(
      alignment: Alignment.topCenter,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: _maxReadableWidth),
        child: ListView(
          padding: const EdgeInsets.all(24),
          // A passage to scroll to must be built, so build the whole transcript then.
          scrollCacheExtent: focusStartMs == null
              ? null
              : const ScrollCacheExtent.pixels(double.maxFinite),
          children: <Widget>[
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Expanded(
                  child: Semantics(
                    header: true,
                    child: Text(
                      summary.title,
                      style: theme.textTheme.headlineMedium
                          ?.copyWith(fontWeight: FontWeight.bold),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                IconButton(
                  tooltip: 'Rename meeting',
                  icon: const Icon(Icons.edit_outlined),
                  onPressed: () => showRenameMeetingDialog(
                    context,
                    controller: controller,
                    meeting: summary,
                  ),
                ),
                IconButton(
                  key: const ValueKey<String>('delete-open-meeting'),
                  tooltip: 'Delete meeting',
                  icon: const Icon(Icons.delete_outline),
                  onPressed: () async {
                    final NavigatorState navigator = Navigator.of(context);
                    final bool deleted = await deleteMeetingAfterAsking(
                      context,
                      controller: controller,
                      meeting: summary,
                    );
                    // On a phone the detail is a page of its own; in two panes it empties.
                    if (deleted && navigator.mounted) {
                      await navigator.maybePop();
                    }
                  },
                ),
              ],
            ),
            const SizedBox(height: 8),
            Text(
              '${formatMeetingCreatedAt(summary.createdAt)} · '
              '${formatMeetingDuration(summary.durationMs)}',
              style: theme.textTheme.bodyMedium
                  ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
            ),
            if (describeTranscriptionLanguages(summary.transcriptionLanguages)
                case final String languages)
              Text(
                languages,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
              ),
            if (summary.recordedOnLabel case final String devices)
              Row(
                children: <Widget>[
                  Icon(Icons.devices_outlined,
                      size: 18, color: theme.colorScheme.onSurfaceVariant),
                  const SizedBox(width: 6),
                  Expanded(
                    child: Text(
                      'Recorded on $devices',
                      style: theme.textTheme.bodyMedium
                          ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                    ),
                  ),
                  TextButton.icon(
                    key: const ValueKey<String>('split-open-meeting'),
                    icon: const Icon(Icons.call_split),
                    label: const Text('Split'),
                    onPressed: () => splitMeetingAfterAsking(
                      context,
                      controller: controller,
                      meeting: summary,
                    ),
                  ),
                ],
              ),
            if (summary.combining case final MeetingCombining combining)
              Padding(
                key: const ValueKey<String>('meeting-combining'),
                padding: const EdgeInsets.only(top: 2, bottom: 4),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Icon(combiningIcon(combining.state),
                        size: 18, color: theme.colorScheme.onSurfaceVariant),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(
                        combining.description,
                        style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant),
                      ),
                    ),
                  ],
                ),
              ),
            SelectableText(
              summary.meetingId,
              style: theme.textTheme.bodySmall
                  ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
            ),
            const Divider(height: 32),
            if (editController
                case final MeetingEditController edits) ...<Widget>[
              MeetingNotesSection(
                  controller: edits, meetingId: summary.meetingId),
              const Divider(height: 32),
            ],
            Text('Transcript', style: theme.textTheme.titleMedium),
            if (editController != null && meeting.segments.isNotEmpty)
              Text(
                'Long-press or right-click a passage to correct it.',
                style: theme.textTheme.bodySmall
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
              ),
            const SizedBox(height: 12),
            ..._transcriptChildren(theme),
          ],
        ),
      ),
    );
  }

  /// Builds one block per segment and attachment in timeline order, or the flat transcript or
  /// an empty-transcript message followed by the attachments.
  List<Widget> _transcriptChildren(ThemeData theme) {
    final int? focusedStart = _focusedStart;
    final List<MeetingTimelineEntry> timeline = buildMeetingTimeline(
        segments: meeting.segments, attachments: meeting.attachments);
    final List<Widget> entries = timeline
        .map((MeetingTimelineEntry entry) => Padding(
              padding: const EdgeInsets.only(bottom: 16),
              child: switch (entry) {
                SegmentEntry(:final MeetingTranscriptSegment segment)
                    when segment.startTimeMs == focusedStart =>
                  _FocusedSegment(
                    key: focusedSegmentKey,
                    child: _editable(theme, segment),
                  ),
                SegmentEntry(:final MeetingTranscriptSegment segment) =>
                  _editable(theme, segment),
                AttachmentEntry(:final MeetingAttachment attachment) =>
                  MeetingAttachmentView(attachment: attachment),
              },
            ))
        .toList(growable: false);
    if (meeting.segments.isNotEmpty) {
      return entries;
    }
    final Widget transcriptText = switch (meeting.transcript) {
      final String transcript when transcript.trim().isNotEmpty =>
        Text(transcript, style: theme.textTheme.bodyLarge),
      _ => Text(
          'No transcript is available.',
          style: theme.textTheme.bodyLarge
              ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
        ),
    };
    return <Widget>[
      Padding(
          padding: const EdgeInsets.only(bottom: 16), child: transcriptText),
      ...entries,
    ];
  }

  /// Builds a segment, correctable when there is an edit controller.
  Widget _editable(ThemeData theme, MeetingTranscriptSegment segment) =>
      switch (editController) {
        final MeetingEditController edits => EditableSegment(
            controller: edits,
            meetings: controller,
            meetingId: meeting.summary.meetingId,
            segment: segment,
            child: _segmentBlock(theme, segment),
          ),
        null => _segmentBlock(theme, segment),
      };

  /// Builds one segment: speaker and start time above its text.
  ///
  /// A guessed speaker reads `Name (?)`, overlapping speech gets a small label, and unclear
  /// words are shown in italics in the outline colour (#90, #91).
  Widget _segmentBlock(ThemeData theme, MeetingTranscriptSegment segment) {
    final String speaker =
        segment.uncertainSpeaker ? '${segment.speaker} (?)' : segment.speaker;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Wrap(
          spacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            _withTooltip(
              segment.uncertainSpeaker ? 'The speaker is a guess' : null,
              Text(
                '$speaker · ${formatMeetingDuration(segment.startTimeMs)}',
                style: theme.textTheme.labelLarge
                    ?.copyWith(color: theme.colorScheme.primary),
              ),
            ),
            if (segment.overlappingSpeech)
              Tooltip(
                message: 'People talked at once here; words may be missing',
                child: Text(
                  'Overlapping speech',
                  style: theme.textTheme.labelSmall
                      ?.copyWith(color: theme.colorScheme.tertiary),
                ),
              ),
          ],
        ),
        const SizedBox(height: 4),
        _segmentText(theme, segment),
      ],
    );
  }

  /// Wraps `child` in a tooltip when there is a `message`.
  Widget _withTooltip(String? message, Widget child) =>
      message == null ? child : Tooltip(message: message, child: child);

  /// Shows a segment's text, with unclear words in italics in the outline colour.
  Widget _segmentText(ThemeData theme, MeetingTranscriptSegment segment) {
    if (segment.uncertainWords.isEmpty) {
      return Text(segment.text, style: theme.textTheme.bodyLarge);
    }
    final Set<int> unclear = segment.uncertainWords.toSet();
    final List<String> words = segment.text.split(RegExp(r'\s+'))
      ..removeWhere((String word) => word.isEmpty);
    final TextStyle unclearStyle = TextStyle(
      color: theme.colorScheme.outline,
      fontStyle: FontStyle.italic,
    );
    return Semantics(
      label: segment.text,
      hint: 'Some words were unclear',
      child: Text.rich(
        TextSpan(
          children: <TextSpan>[
            for (int position = 0;
                position < words.length;
                position++) ...<TextSpan>[
              if (position > 0) const TextSpan(text: ' '),
              TextSpan(
                text: words[position],
                style: unclear.contains(position) ? unclearStyle : null,
              ),
            ],
          ],
        ),
        style: theme.textTheme.bodyLarge,
      ),
    );
  }
}

/// Key of the transcript passage a meeting page was opened at (#100), for tests.
const Key focusedSegmentKey = ValueKey<String>('focused-segment');

/// Marks the passage a meeting page was opened at and scrolls it into view once.
final class _FocusedSegment extends StatefulWidget {
  const _FocusedSegment({required this.child, super.key});

  final Widget child;

  @override
  State<_FocusedSegment> createState() => _FocusedSegmentState();
}

final class _FocusedSegmentState extends State<_FocusedSegment> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        Scrollable.ensureVisible(context,
            alignment: 0.2, duration: const Duration(milliseconds: 300));
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final ColorScheme colors = Theme.of(context).colorScheme;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: colors.tertiaryContainer.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Padding(padding: const EdgeInsets.all(8), child: widget.child),
    );
  }
}

/// Tallest a photo is shown in the transcript, in logical pixels.
const double _maxInlinePhotoHeight = 240;

/// Shows one attachment in the transcript: a photo as a picture that opens full screen, a
/// document as its name and type.
final class MeetingAttachmentView extends StatelessWidget {
  /// Creates the view of `attachment`.
  const MeetingAttachmentView({required this.attachment, super.key});

  final MeetingAttachment attachment;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final String placement = switch (attachment.meetingTimeMs) {
      final int meetingTimeMs => '${attachment.isImage ? 'Photo' : 'File'} · '
          '${formatMeetingDuration(meetingTimeMs)}',
      null => 'Added after the meeting',
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(placement,
            style: theme.textTheme.labelLarge
                ?.copyWith(color: theme.colorScheme.secondary)),
        const SizedBox(height: 4),
        if (attachment.isImage)
          Semantics(
            button: true,
            label: 'Photo ${attachment.fileName}, open full screen',
            child: InkWell(
              onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
                  builder: (BuildContext context) =>
                      _PhotoViewer(attachment: attachment))),
              child: ClipRRect(
                borderRadius: BorderRadius.circular(12),
                child: ConstrainedBox(
                  constraints:
                      const BoxConstraints(maxHeight: _maxInlinePhotoHeight),
                  child: Image.file(
                    File(attachment.filePath),
                    fit: BoxFit.contain,
                    cacheHeight: 720,
                    errorBuilder: (BuildContext context, Object error,
                            StackTrace? stackTrace) =>
                        _DocumentTile(attachment: attachment),
                  ),
                ),
              ),
            ),
          )
        else
          _DocumentTile(attachment: attachment),
      ],
    );
  }
}

/// Shows a document attachment, or a photo that cannot be read, as a card with its name.
final class _DocumentTile extends StatelessWidget {
  const _DocumentTile({required this.attachment});

  final MeetingAttachment attachment;

  @override
  Widget build(BuildContext context) {
    return Card.outlined(
      margin: EdgeInsets.zero,
      child: ListTile(
        leading: Icon(attachment.isImage
            ? Icons.image_outlined
            : Icons.insert_drive_file_outlined),
        title: Text(attachment.fileName),
        subtitle: Text(attachment.mimeType),
      ),
    );
  }
}

/// Shows a photo full screen, with pinch and scroll zoom.
final class _PhotoViewer extends StatelessWidget {
  const _PhotoViewer({required this.attachment});

  final MeetingAttachment attachment;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.black,
      appBar: AppBar(title: Text(attachment.fileName)),
      body: InteractiveViewer(
        maxScale: 5,
        child: Center(child: Image.file(File(attachment.filePath))),
      ),
    );
  }
}

/// The icon for a meeting's combining state (#88).
IconData combiningIcon(CombiningState state) => switch (state) {
      CombiningState.oneDevice => Icons.smartphone_outlined,
      CombiningState.waiting => Icons.sync_outlined,
      CombiningState.readyToCombine => Icons.hourglass_bottom_outlined,
      CombiningState.sending => Icons.upload_outlined,
      CombiningState.combined => Icons.check_circle_outline,
      CombiningState.notCombined => Icons.sync_problem_outlined,
    };
