// Renders one meeting with its title as the header, followed by metadata and transcript, with
// its photos and documents placed where they were taken (Issue #54).

import 'dart:io';

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/controllers/meeting_attachments_controller.dart';
import 'package:note_taker_ui/controllers/meeting_export_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_timeline.dart';
import 'package:note_taker_ui/presentation/delete_meeting_action.dart';
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
    super.key,
  });

  final MeetingListController controller;
  final String meetingId;
  final MeetingAttachmentsController? attachmentsController;
  final MeetingScreenBuilder? photoScreenBuilder;
  final MeetingScreenBuilder? speakersScreenBuilder;
  final MeetingExportController? exportController;

  /// Offers Move to project when given (Issue #85).
  final ProjectsController? projectsController;

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
      body: MeetingDetailView(controller: widget.controller),
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
  /// Creates a view observing `controller`'s selection.
  const MeetingDetailView({required this.controller, super.key});

  final MeetingListController controller;

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
          final MeetingDetail meeting =>
            _MeetingContent(controller: controller, meeting: meeting),
          null => const Center(
              child: Text('Select a meeting to see its transcript.')),
        };
      },
    );
  }
}

/// Lays out the title header (with its rename action), metadata, and transcript of a meeting.
final class _MeetingContent extends StatelessWidget {
  const _MeetingContent({required this.controller, required this.meeting});

  final MeetingListController controller;
  final MeetingDetail meeting;

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
            SelectableText(
              summary.meetingId,
              style: theme.textTheme.bodySmall
                  ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
            ),
            const Divider(height: 32),
            Text('Transcript', style: theme.textTheme.titleMedium),
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
    final List<MeetingTimelineEntry> timeline = buildMeetingTimeline(
        segments: meeting.segments, attachments: meeting.attachments);
    final List<Widget> entries = timeline
        .map((MeetingTimelineEntry entry) => Padding(
              padding: const EdgeInsets.only(bottom: 16),
              child: switch (entry) {
                SegmentEntry(:final MeetingTranscriptSegment segment) =>
                  _segmentBlock(theme, segment),
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

  /// Builds one segment: speaker and start time above its text.
  Widget _segmentBlock(ThemeData theme, MeetingTranscriptSegment segment) =>
      Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(
            '${segment.speaker} · ${formatMeetingDuration(segment.startTimeMs)}',
            style: theme.textTheme.labelLarge
                ?.copyWith(color: theme.colorScheme.primary),
          ),
          const SizedBox(height: 4),
          Text(segment.text, style: theme.textTheme.bodyLarge),
        ],
      );
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
