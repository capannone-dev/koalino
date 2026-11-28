// Renders one meeting with its title as the header, followed by metadata and transcript, with
// its photos and documents placed where they were taken (Issue #54).

import 'dart:async';
import 'dart:io';

// ignore: deprecated_member_use
import 'package:note_taker_ui/presentation/meeting_time.dart';
import 'package:note_taker_ui/controllers/recording_player_controller.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';
import 'package:note_taker_ui/presentation/recording_player_bar.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart' show ScrollCacheExtent, ScrollDirection;
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/presentation/chat_screen.dart';
import 'package:note_taker_ui/presentation/clear_file.dart';
import 'package:note_taker_ui/presentation/photo_gallery.dart';
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
import 'package:note_taker_ui/presentation/readable_width.dart';
import 'package:note_taker_ui/presentation/delete_meeting_action.dart';
import 'package:note_taker_ui/presentation/split_meeting_action.dart';
import 'package:note_taker_ui/presentation/rename_meeting_dialog.dart';
import 'package:note_taker_ui/l10n/l10n.dart';
import 'package:note_taker_ui/infrastructure/speaker_rename_bridge.dart';
import 'package:note_taker_ui/presentation/speaker_label.dart';

/// Builds a screen about one meeting, such as the voice gallery for its unknown speakers.
typedef MeetingScreenBuilder = Widget Function(
    BuildContext context, String meetingId);

/// Widest the detail content grows, in logical pixels, so transcript lines stay readable.
const double _maxReadableWidth = 840;

/// Below this width (logical pixels: phones, narrow windows) the meeting's actions get their own
/// row under the title instead of sharing the title's row.
const double _actionsBesideTitleMinWidth = 600;

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
    this.chatBridge,
    this.recordingsBridge,
    this.speakerRenamer,
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

  /// Adds Chat about this meeting when given (#97).
  final ChatBridge? chatBridge;

  /// Adds the recording player when given and the meeting has a kept recording (#140, #197).
  final RecordingsBridge? recordingsBridge;

  /// Lets the owner rename speakers from the transcript when given (#186).
  final SpeakerRenamer? speakerRenamer;

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

  /// Opens a meeting a chat answer cites, at the cited moment (#110), as a new page over the
  /// chat with this page's tools.
  void _openCitedMeeting(BuildContext context, String meetingId, int? atMs) {
    widget.controller.selectMeeting(meetingId);
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (BuildContext context) => MeetingDetailScreen(
        controller: widget.controller,
        meetingId: meetingId,
        focusStartMs: atMs,
        attachmentsController: widget.attachmentsController,
        photoScreenBuilder: widget.photoScreenBuilder,
        speakersScreenBuilder: widget.speakersScreenBuilder,
        exportController: widget.exportController,
        projectsController: widget.projectsController,
        summariesController: widget.summariesController,
        editController: widget.editController,
        chatBridge: widget.chatBridge,
        recordingsBridge: widget.recordingsBridge,
        speakerRenamer: widget.speakerRenamer,
      ),
    ));
  }

  @override
  Widget build(BuildContext context) {
    final List<Widget> actions = <Widget>[
      if (widget.chatBridge case final ChatBridge bridge)
        IconButton(
          tooltip: l10n.chatAboutThisMeeting,
          icon: const Icon(Icons.chat_outlined),
          onPressed: () => openChat(context,
              bridge: bridge,
              topic: ChatTopic.meeting(widget.meetingId),
              title: l10n.chat(
                  widget.controller.selectedMeeting?.summary.title ??
                      'meeting'),
              openMeeting: _openCitedMeeting,
              projectsController: widget.projectsController,
              handoff: widget.summariesController?.handoff),
        ),
      if (widget.summariesController case final SummariesController summaries)
        ListenableBuilder(
          listenable: widget.controller,
          builder: (BuildContext context, Widget? child) =>
              MeetingSummaryButton(
            controller: summaries,
            meetingId: widget.meetingId,
            title: widget.controller.selectedMeeting?.summary.title ??
                l10n.thisMeeting,
          ),
        ),
      if (widget.speakersScreenBuilder case final MeetingScreenBuilder builder)
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
    ];
    // Up to eight actions do not fit beside the title on a phone: the app bar would slide them
    // over the back button. There they get a row of their own, scrolling sideways if needed.
    final bool narrow =
        MediaQuery.sizeOf(context).width < _actionsBesideTitleMinWidth;
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.meeting2),
        actions: narrow ? null : actions,
        bottom: narrow && actions.isNotEmpty
            ? PreferredSize(
                preferredSize: const Size.fromHeight(kMinInteractiveDimension),
                child: Align(
                  alignment: AlignmentDirectional.centerEnd,
                  child: SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    padding: const EdgeInsetsDirectional.only(end: 4),
                    child: Row(children: actions),
                  ),
                ),
              )
            : null,
      ),
      body: MeetingDetailView(
          controller: widget.controller,
          editController: widget.editController,
          speakerRenamer: widget.speakerRenamer,
          recordingsBridge: widget.recordingsBridge,
          exportController: widget.exportController,
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
      tooltip: l10n.takePhotos,
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
        tooltip: l10n.attachFiles,
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
  /// Side effects: Runs the export, which may show a save dialog, or shares it (where the
  /// export controller can share: "Share as …" entries), then shows a snack bar (nothing when
  /// the owner cancels the dialog or the share sheet opened).
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
          PopupMenuButton<(MeetingExportFormat, bool)>(
        tooltip: l10n.export,
        enabled: !exportController.isExporting,
        icon: exportController.isExporting
            ? const SizedBox.square(
                dimension: 20, child: CircularProgressIndicator(strokeWidth: 2))
            : const Icon(Icons.file_download_outlined),
        onSelected: ((MeetingExportFormat, bool) choice) =>
            _export(context, choice.$1, share: choice.$2),
        itemBuilder: (BuildContext context) =>
            <PopupMenuEntry<(MeetingExportFormat, bool)>>[
          for (final MeetingExportFormat format in MeetingExportFormat.values)
            PopupMenuItem<(MeetingExportFormat, bool)>(
                value: (format, false), child: Text(format.label)),
          if (exportController
              .canShare) ...<PopupMenuEntry<(MeetingExportFormat, bool)>>[
            const PopupMenuDivider(),
            for (final MeetingExportFormat format in MeetingExportFormat.values)
              PopupMenuItem<(MeetingExportFormat, bool)>(
                  value: (format, true),
                  child: Text(l10n.shareAs(format.label))),
          ],
        ],
      ),
    );
  }

  /// Runs the export, or shares it when `share`, and reports its result.
  Future<void> _export(BuildContext context, MeetingExportFormat format,
      {required bool share}) async {
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final String title =
        meetingController.selectedMeeting?.summary.title ?? 'meeting';
    final String? message = share
        ? await exportController.shareMeeting(
            meetingId: meetingId, title: title, format: format)
        : await exportController.exportMeeting(
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
      tooltip: l10n.nameSpeakers,
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
      this.speakerRenamer,
      this.recordingsBridge,
      this.exportController,
      this.focusStartMs,
      super.key});

  final MeetingListController controller;
  final MeetingEditController? editController;

  /// Lets the owner rename speakers from the transcript when given (#186).
  final SpeakerRenamer? speakerRenamer;

  /// Adds the recording player under the transcript when given and the meeting keeps its
  /// recording (#197).
  final RecordingsBridge? recordingsBridge;

  /// Adds sharing (Android) or saving (desktops) the kept recording to the player when given
  /// (#199).
  final MeetingExportController? exportController;

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
          final MeetingDetail meeting => switch (recordingsBridge) {
              final RecordingsBridge bridge => _MeetingWithPlayer(
                  // A new meeting gets a new player; the old one releases its file.
                  key: ValueKey<String>(meeting.summary.meetingId),
                  bridge: bridge,
                  meeting: meeting,
                  exportController: exportController,
                  contentBuilder: (RecordingPlayerController? player) =>
                      _MeetingContent(
                          controller: controller,
                          meeting: meeting,
                          editController: editController,
                          speakerRenamer: speakerRenamer,
                          player: player,
                          focusStartMs: focusStartMs),
                ),
              null => _MeetingContent(
                  controller: controller,
                  meeting: meeting,
                  editController: editController,
                  speakerRenamer: speakerRenamer,
                  focusStartMs: focusStartMs),
            },
          null => Center(child: Text(l10n.selectAMeetingToSeeIts)),
        };
      },
    );
  }
}

/// Owns a meeting's recording player (#197): the transcript above, the player bar below.
///
/// The transcript gets the player only once a recording is known to be kept, so a meeting without
/// one looks as before. Side effects: Asks the core once whether a recording is kept; closing the
/// page releases the decrypted file.
final class _MeetingWithPlayer extends StatefulWidget {
  const _MeetingWithPlayer({
    required this.bridge,
    required this.meeting,
    required this.contentBuilder,
    this.exportController,
    super.key,
  });

  final RecordingsBridge bridge;
  final MeetingDetail meeting;
  final MeetingExportController? exportController;
  final Widget Function(RecordingPlayerController? player) contentBuilder;

  @override
  State<_MeetingWithPlayer> createState() => _MeetingWithPlayerState();
}

final class _MeetingWithPlayerState extends State<_MeetingWithPlayer> {
  late final RecordingPlayerController _player = RecordingPlayerController(
    bridge: widget.bridge,
    meetingId: widget.meeting.summary.meetingId,
  );
  bool _available = false;

  @override
  void initState() {
    super.initState();
    _setPassages();
    _player.addListener(_onPlayerChanged);
    unawaited(_player.check());
  }

  @override
  void didUpdateWidget(_MeetingWithPlayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.meeting, widget.meeting)) {
      _setPassages();
    }
  }

  void _setPassages() => _player.passageStarts = <int>[
        for (final MeetingTranscriptSegment segment in widget.meeting.segments)
          segment.startTimeMs,
      ];

  /// Rebuilds the transcript only when the player appears, not at every position.
  void _onPlayerChanged() {
    if (_player.available != _available && mounted) {
      setState(() => _available = _player.available);
    }
  }

  /// Shares or saves the recording (#199) and shows the result, if any, in a snack bar.
  Future<void> _exportRecording(
      BuildContext context, MeetingExportController exports) async {
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final String? message = await exports.exportRecording(
      meetingId: widget.meeting.summary.meetingId,
      title: widget.meeting.summary.title,
      recordings: widget.bridge,
    );
    if (message != null) {
      messenger.showSnackBar(SnackBar(content: Text(message)));
    }
  }

  @override
  void dispose() {
    _player.removeListener(_onPlayerChanged);
    _player.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Expanded(child: widget.contentBuilder(_available ? _player : null)),
        RecordingPlayerBar(
          player: _player,
          fallbackDurationMs: widget.meeting.summary.durationMs,
          exportAction: switch (widget.exportController) {
            final MeetingExportController exports => ListenableBuilder(
                listenable: exports,
                builder: (BuildContext context, Widget? child) => IconButton(
                  key: recordingExportKey,
                  tooltip: exports.canShare
                      ? l10n.shareTheRecording
                      : l10n.saveTheRecording,
                  icon: exports.isExporting
                      ? const SizedBox.square(
                          dimension: 20,
                          child: CircularProgressIndicator(strokeWidth: 2))
                      : Icon(exports.canShare
                          ? Icons.share_outlined
                          : Icons.save_alt),
                  onPressed: exports.isExporting
                      ? null
                      : () => _exportRecording(context, exports),
                ),
              ),
            null => null,
          },
        ),
      ],
    );
  }
}

/// Lays out the title header (with its rename action), metadata, and transcript of a meeting.
final class _MeetingContent extends StatelessWidget {
  const _MeetingContent(
      {required this.controller,
      required this.meeting,
      this.editController,
      this.speakerRenamer,
      this.player,
      this.focusStartMs});

  final MeetingListController controller;
  final MeetingDetail meeting;
  final MeetingEditController? editController;
  final SpeakerRenamer? speakerRenamer;

  /// The kept recording's player, when there is one: the passage being heard is marked and kept
  /// in view, and each passage can be played from its start (#197).
  final RecordingPlayerController? player;
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
    // The list fills the pane so the wheel scrolls anywhere across it (#142).
    final Widget content = ReadableWidth(
      maxWidth: _maxReadableWidth,
      builder: (BuildContext context, EdgeInsets padding) => ListView(
        padding: padding,
        // A passage to scroll to must be built, so build the whole transcript then; the player
        // can move to any passage.
        scrollCacheExtent: focusStartMs == null && player == null
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
                tooltip: l10n.renameMeeting,
                icon: const Icon(Icons.edit_outlined),
                onPressed: () => showRenameMeetingDialog(
                  context,
                  controller: controller,
                  meeting: summary,
                ),
              ),
              IconButton(
                key: const ValueKey<String>('delete-open-meeting'),
                tooltip: l10n.deleteMeeting,
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
            '${formatMeetingWhen(summary.createdAt)} · '
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
                    l10n.recordedOn(devices),
                    style: theme.textTheme.bodyMedium
                        ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                  ),
                ),
                TextButton.icon(
                  key: const ValueKey<String>('split-open-meeting'),
                  icon: const Icon(Icons.call_split),
                  label: Text(l10n.split),
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
                      style: theme.textTheme.bodySmall
                          ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
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
          Text(l10n.transcript, style: theme.textTheme.titleMedium),
          if (editController != null && meeting.segments.isNotEmpty)
            Text(
              l10n.longPressOrRightClickA,
              style: theme.textTheme.bodySmall
                  ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
            ),
          const SizedBox(height: 12),
          ..._transcriptChildren(theme),
        ],
      ),
    );
    final RecordingPlayerController? recordingPlayer = player;
    if (recordingPlayer == null) {
      return content;
    }
    // Scrolling by hand while it plays stops following; the player bar offers to follow again.
    return NotificationListener<UserScrollNotification>(
      onNotification: (UserScrollNotification notification) {
        if (recordingPlayer.playing &&
            notification.direction != ScrollDirection.idle) {
          recordingPlayer.stopFollowing();
        }
        return false;
      },
      child: content,
    );
  }

  /// Builds one block per segment and attachment in timeline order, or the flat transcript or
  /// an empty-transcript message followed by the attachments.
  List<Widget> _transcriptChildren(ThemeData theme) {
    final int? focusedStart = _focusedStart;
    final List<MeetingTimelineEntry> timeline = buildMeetingTimeline(
        segments: meeting.segments, attachments: meeting.attachments);
    // The meeting's photos in timeline order: the full-screen viewer moves between them (#144).
    final List<MeetingAttachment> photos = <MeetingAttachment>[
      for (final MeetingTimelineEntry entry in timeline)
        if (entry case AttachmentEntry(:final MeetingAttachment attachment)
            when attachment.isImage)
          attachment,
    ];
    final List<Widget> entries = timeline
        .map((MeetingTimelineEntry entry) => Padding(
              padding: const EdgeInsets.only(bottom: 16),
              child: switch (entry) {
                SegmentEntry(:final MeetingTranscriptSegment segment)
                    when segment.startTimeMs == focusedStart =>
                  _FocusedSegment(
                    key: focusedSegmentKey,
                    child: _heard(segment, _editable(theme, segment)),
                  ),
                SegmentEntry(:final MeetingTranscriptSegment segment) =>
                  _heard(segment, _editable(theme, segment)),
                AttachmentEntry(:final MeetingAttachment attachment) =>
                  MeetingAttachmentView(attachment: attachment, photos: photos),
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
          // A meeting started by writing has no recording at all (#129).
          meeting.summary.durationMs == 0
              ? l10n.noRecordingWriteNotesOrAttach
              : l10n.noTranscriptIsAvailable,
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

  /// Marks `child` while the player is in its passage, when there is a player.
  Widget _heard(MeetingTranscriptSegment segment, Widget child) =>
      switch (player) {
        final RecordingPlayerController player => _HeardPassage(
            player: player, startMs: segment.startTimeMs, child: child),
        null => child,
      };

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
              segment.uncertainSpeaker ? l10n.theSpeakerIsAGuess : null,
              _speakerLabel(
                theme,
                segment,
                '$speaker · ${formatMeetingDuration(segment.startTimeMs)}',
              ),
            ),
            if (player case final RecordingPlayerController player)
              IconButton(
                key: ValueKey<String>('play-from-${segment.startTimeMs}'),
                tooltip: l10n.playFromHere,
                visualDensity: VisualDensity.compact,
                iconSize: 18,
                icon: const Icon(Icons.play_circle_outline),
                onPressed: () => player.playFrom(segment.startTimeMs),
              ),
            if (segment.overlappingSpeech)
              Tooltip(
                message: l10n.peopleTalkedAtOnceHereWords,
                child: Text(
                  l10n.overlappingSpeech,
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

  /// The segment's speaker label, renamable by double-click or long-press when there is a
  /// renamer and the segment has a voice (#186).
  Widget _speakerLabel(
      ThemeData theme, MeetingTranscriptSegment segment, String text) {
    final TextStyle? style =
        theme.textTheme.labelLarge?.copyWith(color: theme.colorScheme.primary);
    final SpeakerRenamer? renamer = speakerRenamer;
    if (renamer == null) {
      return Text(text, style: style);
    }
    return SpeakerLabel(
      text: text,
      name: segment.speaker,
      voice: segment.voice,
      renamer: renamer,
      style: style,
      // Every passage by this voice changes, so the whole meeting reloads.
      onRenamed: (_) => unawaited(controller.refreshSelectedMeeting()),
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
      hint: l10n.someWordsWereUnclear,
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

/// Marks the passage being heard and, while following, keeps it in view (#197).
///
/// It listens only to which passage is current, so it rebuilds when the player enters or leaves
/// this passage, not at every position update.
final class _HeardPassage extends StatelessWidget {
  const _HeardPassage(
      {required this.player, required this.startMs, required this.child});

  final RecordingPlayerController player;
  final int startMs;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<int?>(
      valueListenable: player.currentPassageStart,
      builder: (BuildContext context, int? current, Widget? child) {
        final bool heard = current == startMs;
        if (heard && player.follow && player.playing) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (context.mounted) {
              Scrollable.ensureVisible(context,
                  alignment: 0.3, duration: const Duration(milliseconds: 300));
            }
          });
        }
        final ColorScheme colors = Theme.of(context).colorScheme;
        return AnimatedContainer(
          key: heard ? heardPassageKey : null,
          duration: const Duration(milliseconds: 200),
          decoration: BoxDecoration(
            color: heard
                ? colors.secondaryContainer.withValues(alpha: 0.6)
                : Colors.transparent,
            borderRadius: BorderRadius.circular(8),
          ),
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          child: child,
        );
      },
      child: child,
    );
  }
}

/// Key of the transcript passage being heard (#197), for tests.
const Key heardPassageKey = ValueKey<String>('heard-passage');

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
  ///
  /// Inputs: The attachment, and the meeting's photos in order (the full-screen viewer moves
  /// between them; just this one when absent). Outputs: A widget. Side effects: None.
  const MeetingAttachmentView(
      {required this.attachment, this.photos, super.key});

  final MeetingAttachment attachment;

  /// The meeting's photos, in the order the viewer shows them.
  final List<MeetingAttachment>? photos;

  /// Opens the full-screen viewer on this photo. Side effects: Pushes a route.
  void _openViewer(BuildContext context) {
    final List<MeetingAttachment> all = switch (photos) {
      final List<MeetingAttachment> photos
          when photos.any((MeetingAttachment photo) =>
              photo.attachmentId == attachment.attachmentId) =>
        photos,
      _ => <MeetingAttachment>[attachment],
    };
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (BuildContext context) => PhotoGallery(
        count: all.length,
        initialIndex: all.indexWhere((MeetingAttachment photo) =>
            photo.attachmentId == attachment.attachmentId),
        titleOf: (int index) => all[index].fileName,
        photoBuilder: (BuildContext context, int index) => ClearFile(
          attachmentId: all[index].attachmentId,
          unavailableBuilder: (BuildContext context) => const Icon(
              Icons.broken_image_outlined,
              color: Colors.white70,
              size: 48),
          builder: (BuildContext context, String path) =>
              Image.file(File(path)),
        ),
      ),
    ));
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final String placement = switch (attachment.meetingTimeMs) {
      final int meetingTimeMs => '${attachment.isImage ? 'Photo' : 'File'} · '
          '${formatMeetingDuration(meetingTimeMs)}',
      null => l10n.addedAfterTheMeeting,
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
            label: l10n.photoOpenFullScreen(attachment.fileName),
            child: InkWell(
              onTap: () => _openViewer(context),
              child: ClipRRect(
                borderRadius: BorderRadius.circular(12),
                child: ConstrainedBox(
                  constraints:
                      const BoxConstraints(maxHeight: _maxInlinePhotoHeight),
                  // The file is sealed; the picture comes from a clear copy (#99).
                  child: ClearFile(
                    attachmentId: attachment.attachmentId,
                    unavailableBuilder: (BuildContext context) =>
                        _DocumentTile(attachment: attachment),
                    builder: (BuildContext context, String path) => Image.file(
                      File(path),
                      fit: BoxFit.contain,
                      cacheHeight: 720,
                      errorBuilder: (BuildContext context, Object error,
                              StackTrace? stackTrace) =>
                          _DocumentTile(attachment: attachment),
                    ),
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

/// The icon for a meeting's combining state (#88).
IconData combiningIcon(CombiningState state) => switch (state) {
      CombiningState.oneDevice => Icons.smartphone_outlined,
      CombiningState.waiting => Icons.sync_outlined,
      CombiningState.readyToCombine => Icons.hourglass_bottom_outlined,
      CombiningState.sending => Icons.upload_outlined,
      CombiningState.combined => Icons.check_circle_outline,
      CombiningState.notCombined => Icons.sync_problem_outlined,
    };
