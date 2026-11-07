// Renders the meeting list title-first, as a phone list or a desktop list-detail layout.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/controllers/meeting_attachments_controller.dart';
import 'package:note_taker_ui/controllers/meeting_export_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/delete_meeting_action.dart';
import 'package:note_taker_ui/presentation/rename_meeting_dialog.dart';

/// Window width from which the list and the detail are shown side by side (Material 3
/// "expanded" window class).
const double twoPaneBreakpoint = 840;

/// Width of the list pane in the two-pane layout.
const double _listPaneWidth = 360;

/// Lists meetings and opens their detail: as a route on phones, beside the list on desktops.
final class MeetingListScreen extends StatefulWidget {
  /// Creates a screen observing `controller`.
  ///
  /// Inputs: The meeting controller, the attachments controller, optional builders for the
  /// camera and a meeting's speakers, and the export controller, passed on to the detail.
  /// Outputs: A Material 3 meeting list screen.
  /// Side effects: Loads the meetings when first inserted.
  const MeetingListScreen({
    required this.controller,
    this.attachmentsController,
    this.photoScreenBuilder,
    this.speakersScreenBuilder,
    this.exportController,
    this.projectsController,
    super.key,
  });

  final MeetingListController controller;
  final MeetingAttachmentsController? attachmentsController;
  final MeetingScreenBuilder? photoScreenBuilder;
  final MeetingScreenBuilder? speakersScreenBuilder;
  final MeetingExportController? exportController;

  /// Offers Move to project for the open meeting when given (Issue #85).
  final ProjectsController? projectsController;

  @override
  State<MeetingListScreen> createState() => _MeetingListScreenState();
}

/// Requests the meetings once and chooses the layout from the available width.
final class _MeetingListScreenState extends State<MeetingListScreen> {
  @override
  void initState() {
    super.initState();
    widget.controller.loadMeetings();
  }

  /// Opens `meeting`: selects it, then on phones also pushes its detail route.
  void _openMeeting(MeetingSummary meeting, {required bool isTwoPane}) {
    widget.controller.selectMeeting(meeting.meetingId);
    if (isTwoPane) {
      return;
    }
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (BuildContext context) => MeetingDetailScreen(
        controller: widget.controller,
        meetingId: meeting.meetingId,
        attachmentsController: widget.attachmentsController,
        photoScreenBuilder: widget.photoScreenBuilder,
        speakersScreenBuilder: widget.speakersScreenBuilder,
        exportController: widget.exportController,
        projectsController: widget.projectsController,
      ),
    ));
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final bool isTwoPane = constraints.maxWidth >= twoPaneBreakpoint;
        final Widget meetingList = _MeetingList(
          controller: widget.controller,
          highlightSelection: isTwoPane,
          onOpenMeeting: (MeetingSummary meeting) =>
              _openMeeting(meeting, isTwoPane: isTwoPane),
        );
        return Scaffold(
          appBar: AppBar(
            title: const Text('Meetings'),
            actions: <Widget>[
              IconButton(
                tooltip: 'Refresh',
                icon: const Icon(Icons.refresh),
                onPressed: widget.controller.loadMeetings,
              ),
              if (isTwoPane)
                if (widget.speakersScreenBuilder
                    case final MeetingScreenBuilder builder)
                  AnimatedBuilder(
                    animation: widget.controller,
                    builder: (BuildContext context, Widget? child) =>
                        switch (widget.controller.selectedMeetingId) {
                      final String meetingId
                          when widget.controller.selectedMeeting != null =>
                        NameSpeakersButton(
                          controller: widget.controller,
                          meetingId: meetingId,
                          speakersScreenBuilder: builder,
                        ),
                      _ => const SizedBox.shrink(),
                    },
                  ),
              if (isTwoPane)
                if (widget.exportController
                    case final MeetingExportController exportController)
                  AnimatedBuilder(
                    animation: widget.controller,
                    builder: (BuildContext context, Widget? child) =>
                        switch (widget.controller.selectedMeetingId) {
                      final String meetingId
                          when widget.controller.selectedMeeting != null =>
                        ExportMeetingButton(
                          meetingController: widget.controller,
                          exportController: exportController,
                          meetingId: meetingId,
                        ),
                      _ => const SizedBox.shrink(),
                    },
                  ),
              if (isTwoPane)
                if (widget.photoScreenBuilder
                    case final MeetingScreenBuilder builder)
                  AnimatedBuilder(
                    animation: widget.controller,
                    builder: (BuildContext context, Widget? child) =>
                        switch (widget.controller.selectedMeetingId) {
                      final String meetingId
                          when widget.controller.selectedMeeting != null =>
                        TakePhotosButton(
                          controller: widget.controller,
                          meetingId: meetingId,
                          photoScreenBuilder: builder,
                        ),
                      _ => const SizedBox.shrink(),
                    },
                  ),
              if (isTwoPane)
                if (widget.attachmentsController
                    case final MeetingAttachmentsController
                        attachmentsController)
                  AnimatedBuilder(
                    animation: widget.controller,
                    builder: (BuildContext context, Widget? child) =>
                        switch (widget.controller.selectedMeetingId) {
                      final String meetingId
                          when widget.controller.selectedMeeting != null =>
                        AttachFilesButton(
                          controller: widget.controller,
                          attachmentsController: attachmentsController,
                          meetingId: meetingId,
                        ),
                      _ => const SizedBox.shrink(),
                    },
                  ),
              if (isTwoPane)
                if (widget.projectsController
                    case final ProjectsController projectsController)
                  AnimatedBuilder(
                    animation: widget.controller,
                    builder: (BuildContext context, Widget? child) =>
                        switch (widget.controller.selectedMeetingId) {
                      final String meetingId
                          when widget.controller.selectedMeeting != null =>
                        MoveToProjectButton(
                          controller: projectsController,
                          meetingId: meetingId,
                        ),
                      _ => const SizedBox.shrink(),
                    },
                  ),
            ],
          ),
          body: isTwoPane
              ? Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    SizedBox(width: _listPaneWidth, child: meetingList),
                    const VerticalDivider(width: 1),
                    Expanded(
                        child:
                            MeetingDetailView(controller: widget.controller)),
                  ],
                )
              : meetingList,
        );
      },
    );
  }
}

/// Renders the loading, error, empty, or populated meeting list.
final class _MeetingList extends StatelessWidget {
  const _MeetingList({
    required this.controller,
    required this.highlightSelection,
    required this.onOpenMeeting,
  });

  final MeetingListController controller;

  /// Whether the selected meeting is highlighted (only when its detail is visible beside it).
  final bool highlightSelection;
  final ValueChanged<MeetingSummary> onOpenMeeting;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return AnimatedBuilder(
      animation: controller,
      builder: (BuildContext context, Widget? child) {
        if (controller.isLoading && controller.meetings.isEmpty) {
          return const Center(child: CircularProgressIndicator());
        }
        if (controller.errorMessage case final String errorMessage) {
          return Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Text(errorMessage),
                const SizedBox(height: 12),
                FilledButton.tonal(
                  onPressed: controller.loadMeetings,
                  child: const Text('Retry'),
                ),
              ],
            ),
          );
        }
        if (controller.meetings.isEmpty) {
          return Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Icon(Icons.event_note,
                    size: 48, color: theme.colorScheme.onSurfaceVariant),
                const SizedBox(height: 12),
                Text('No meetings yet', style: theme.textTheme.titleMedium),
                const SizedBox(height: 4),
                const Text('Recordings you start appear here.'),
              ],
            ),
          );
        }
        final List<MeetingSummary> newestFirst =
            controller.meetings.reversed.toList(growable: false);
        return RefreshIndicator(
          onRefresh: controller.loadMeetings,
          child: ListView.builder(
            itemCount: newestFirst.length,
            itemBuilder: (BuildContext context, int index) {
              final MeetingSummary meeting = newestFirst[index];
              return _MeetingRow(
                controller: controller,
                meeting: meeting,
                child: ListTile(
                  leading: const Icon(Icons.event_note),
                  selected: highlightSelection &&
                      controller.selectedMeetingId == meeting.meetingId,
                  title: Text(
                    meeting.title,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.titleMedium
                        ?.copyWith(fontWeight: FontWeight.w600),
                  ),
                  subtitle: Text(
                    '${formatMeetingCreatedAt(meeting.createdAt)} · '
                    '${formatMeetingDuration(meeting.durationMs)}'
                    '${meeting.recordedOnLabel == null ? '' : ' · ${meeting.recordedOnLabel}'}',
                  ),
                  trailing: IconButton(
                    key: ValueKey<String>('delete-${meeting.meetingId}'),
                    tooltip: 'Delete meeting',
                    icon: const Icon(Icons.delete_outline),
                    onPressed: () => deleteMeetingAfterAsking(
                      context,
                      controller: controller,
                      meeting: meeting,
                    ),
                  ),
                  onTap: () => onOpenMeeting(meeting),
                ),
              );
            },
          ),
        );
      },
    );
  }
}

/// Actions offered in a meeting row's context menu.
enum _MeetingRowAction {
  /// Opens the rename dialog.
  rename,
}

/// Adds a context menu with "Rename" to one meeting row.
///
/// The menu opens on long-press (touch), right-click (mouse), and Shift+F10 or the context menu
/// key (keyboard); screen readers get the same action as a custom semantics action.
final class _MeetingRow extends StatelessWidget {
  const _MeetingRow({
    required this.controller,
    required this.meeting,
    required this.child,
  });

  final MeetingListController controller;
  final MeetingSummary meeting;

  /// The row's list tile.
  final Widget child;

  /// Shows the context menu at `globalPosition` and runs the chosen action.
  ///
  /// Inputs: A context inside the row and the pointer position in global coordinates.
  /// Outputs: Completes once the menu (and any dialog it opened) closes.
  /// Side effects: May rename the meeting through the rename dialog.
  Future<void> _showContextMenu(
      BuildContext context, Offset globalPosition) async {
    if (Overlay.of(context).context.findRenderObject()
        case final RenderBox overlay) {
      await _showContextMenuIn(context, overlay, globalPosition);
    }
  }

  /// Shows the context menu inside `overlay` and runs the chosen action.
  Future<void> _showContextMenuIn(
      BuildContext context, RenderBox overlay, Offset globalPosition) async {
    final Offset overlayPosition = overlay.globalToLocal(globalPosition);
    final _MeetingRowAction? action = await showMenu<_MeetingRowAction>(
      context: context,
      position: RelativeRect.fromRect(
          overlayPosition & Size.zero, Offset.zero & overlay.size),
      items: const <PopupMenuEntry<_MeetingRowAction>>[
        PopupMenuItem<_MeetingRowAction>(
          value: _MeetingRowAction.rename,
          child: ListTile(
            leading: Icon(Icons.edit_outlined),
            title: Text('Rename'),
          ),
        ),
      ],
    );
    if (action == _MeetingRowAction.rename && context.mounted) {
      await _rename(context);
    }
  }

  /// Opens the context menu below the row's centre, for keyboard users.
  void _showContextMenuForRow(BuildContext context) {
    if (context.findRenderObject() case final RenderBox rowBox) {
      _showContextMenu(
          context, rowBox.localToGlobal(rowBox.size.center(Offset.zero)));
    }
  }

  /// Opens the rename dialog for this row's meeting.
  Future<void> _rename(BuildContext context) => showRenameMeetingDialog(
        context,
        controller: controller,
        meeting: meeting,
      );

  @override
  Widget build(BuildContext context) {
    return Semantics(
      customSemanticsActions: <CustomSemanticsAction, VoidCallback>{
        const CustomSemanticsAction(label: 'Rename'): () => _rename(context),
        const CustomSemanticsAction(label: 'Delete'): () =>
            deleteMeetingAfterAsking(
              context,
              controller: controller,
              meeting: meeting,
            ),
      },
      child: CallbackShortcuts(
        bindings: <ShortcutActivator, VoidCallback>{
          const SingleActivator(LogicalKeyboardKey.f10, shift: true): () =>
              _showContextMenuForRow(context),
          const SingleActivator(LogicalKeyboardKey.contextMenu): () =>
              _showContextMenuForRow(context),
        },
        child: GestureDetector(
          onLongPressStart: (LongPressStartDetails details) =>
              _showContextMenu(context, details.globalPosition),
          onSecondaryTapUp: (TapUpDetails details) =>
              _showContextMenu(context, details.globalPosition),
          child: child,
        ),
      ),
    );
  }
}
