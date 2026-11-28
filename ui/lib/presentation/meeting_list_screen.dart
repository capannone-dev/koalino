// Renders the meeting list title-first, as a phone list or a desktop list-detail layout.

// ignore: deprecated_member_use
import 'package:note_taker_ui/presentation/meeting_time.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';
import 'package:flutter/material.dart';
import 'package:note_taker_ui/presentation/keyboard_shortcuts.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/controllers/meeting_edit_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/infrastructure/search_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_summary_screen.dart';
import 'package:note_taker_ui/presentation/chat_screen.dart';
import 'package:note_taker_ui/presentation/search_screen.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/controllers/meeting_attachments_controller.dart';
import 'package:note_taker_ui/controllers/meeting_export_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/speaker_rename_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/delete_meeting_action.dart';
import 'package:note_taker_ui/presentation/confirm_delete_dialog.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/presentation/home_shell.dart';
import 'package:note_taker_ui/presentation/new_meeting_dialog.dart';
import 'package:note_taker_ui/presentation/rename_meeting_dialog.dart';
import 'package:note_taker_ui/presentation/resizable_pane.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Window width from which the list and the detail are shown side by side (Material 3
/// "expanded" window class).
const double twoPaneBreakpoint = 840;

/// Default width of the list pane in the two-pane layout; the owner can drag it (#142).
const double _listPaneWidth = 360;

/// Room always left to the open meeting beside the list.
const double _minDetailWidth = 400;

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
    this.summariesController,
    this.editController,
    this.speakerRenamer,
    this.searchBridge,
    this.chatBridge,
    this.recordingsBridge,
    super.key,
  });

  final MeetingListController controller;
  final MeetingAttachmentsController? attachmentsController;
  final MeetingScreenBuilder? photoScreenBuilder;
  final MeetingScreenBuilder? speakersScreenBuilder;
  final MeetingExportController? exportController;

  /// Offers Move to project for the open meeting when given (Issue #85).
  final ProjectsController? projectsController;

  /// Adds a summary button and menu to each meeting, and to the open one, when given (#95).
  final SummariesController? summariesController;

  /// Shows the open meeting's notes and lets the owner correct its transcript when given (#96).
  final MeetingEditController? editController;

  /// Lets the owner rename speakers from the transcript when given (#186).
  final SpeakerRenamer? speakerRenamer;

  /// Adds Search over every meeting when given (#100).
  final SearchBridge? searchBridge;

  /// Adds the chat over every meeting, summary and project, and a meeting's chat (#97).
  final ChatBridge? chatBridge;

  /// Lets a meeting play its kept recording (#140).
  final RecordingsBridge? recordingsBridge;

  @override
  State<MeetingListScreen> createState() => _MeetingListScreenState();
}

/// Requests the meetings once and chooses the layout from the available width.
final class _MeetingListScreenState extends State<MeetingListScreen> {
  /// The meetings chosen for a batch action (#216); empty outside selection mode.
  final Set<String> _selection = <String>{};

  @override
  void initState() {
    super.initState();
    widget.controller.loadMeetings();
  }

  /// Adds `meeting` to the selection, or removes it.
  void _toggleSelected(MeetingSummary meeting) => setState(() {
        if (!_selection.remove(meeting.meetingId)) {
          _selection.add(meeting.meetingId);
        }
      });

  /// Leaves selection mode.
  void _clearSelection() => setState(_selection.clear);

  /// The selected meetings still in the list, newest first.
  List<MeetingSummary> get _selectedMeetings => <MeetingSummary>[
        for (final MeetingSummary meeting
            in widget.controller.meetings.reversed)
          if (_selection.contains(meeting.meetingId)) meeting,
      ];

  /// Deletes the selected meetings once the owner confirms.
  ///
  /// Side effects: Shows a dialog; deletes through the controller; a snack bar on refusals.
  Future<void> _deleteSelected() async {
    final List<MeetingSummary> meetings = _selectedMeetings;
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final bool confirmed = await confirmDelete(
      context,
      title: l10n.deleteMeetingsCount(meetings.length),
      message: l10n.itsTranscriptAttachmentsAndUnknownVoices,
    );
    if (!confirmed) {
      return;
    }
    int refused = 0;
    for (final MeetingSummary meeting in meetings) {
      if (!await widget.controller.deleteMeeting(meeting.meetingId)) {
        refused++;
      }
    }
    if (refused > 0) {
      messenger.showSnackBar(
          SnackBar(content: Text(l10n.unableToDeleteThisMeetingIf)));
    }
    if (mounted) {
      _clearSelection();
    }
  }

  /// Exports the selected meetings in `format` and reports the outcome.
  Future<void> _exportSelected(
      MeetingExportController exports, MeetingExportFormat format) async {
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final String? message =
        await exports.exportMeetings(<({String meetingId, String title})>[
      for (final MeetingSummary meeting in _selectedMeetings)
        (meetingId: meeting.meetingId, title: meeting.title),
    ], format);
    if (message != null) {
      messenger.showSnackBar(SnackBar(content: Text(message)));
    }
  }

  /// The app bar of selection mode (#216): how many are chosen, and what can be done to them.
  AppBar _selectionBar() {
    final List<MeetingSummary> meetings = _selectedMeetings;
    return AppBar(
      key: meetingSelectionBarKey,
      leading: IconButton(
        tooltip: l10n.cancelSelection,
        icon: const Icon(Icons.close),
        onPressed: _clearSelection,
      ),
      title: Text(l10n.selectedCount(meetings.length)),
      actions: <Widget>[
        IconButton(
          tooltip: l10n.selectAll,
          icon: const Icon(Icons.select_all),
          onPressed: () => setState(() => _selection.addAll(<String>[
                for (final MeetingSummary meeting in widget.controller.meetings)
                  meeting.meetingId,
              ])),
        ),
        if (widget.projectsController case final ProjectsController projects)
          IconButton(
            tooltip: l10n.moveToProject,
            icon: const Icon(Icons.drive_file_move_outline),
            onPressed: meetings.isEmpty
                ? null
                : () async {
                    await chooseProjectForMeetings(context,
                        controller: projects,
                        meetingIds: <String>[
                          for (final MeetingSummary meeting in meetings)
                            meeting.meetingId,
                        ]);
                    if (mounted) {
                      _clearSelection();
                    }
                  },
          ),
        if (widget.exportController case final MeetingExportController exports)
          PopupMenuButton<MeetingExportFormat>(
            tooltip: exports.canShare ? l10n.shareAction : l10n.export,
            icon: Icon(exports.canShare
                ? Icons.share_outlined
                : Icons.file_download_outlined),
            enabled: meetings.isNotEmpty,
            onSelected: (MeetingExportFormat format) =>
                _exportSelected(exports, format),
            itemBuilder: (BuildContext context) =>
                <PopupMenuEntry<MeetingExportFormat>>[
              for (final MeetingExportFormat format
                  in MeetingExportFormat.values)
                PopupMenuItem<MeetingExportFormat>(
                    value: format, child: Text(format.label)),
            ],
          ),
        IconButton(
          key: const ValueKey<String>('delete-selected'),
          tooltip: l10n.delete,
          icon: const Icon(Icons.delete_outline),
          onPressed: meetings.isEmpty ? null : _deleteSelected,
        ),
      ],
    );
  }

  /// Opens `meeting`: selects it, then on phones also pushes its detail route.
  void _openMeeting(MeetingSummary meeting, {required bool isTwoPane}) {
    widget.controller.selectMeeting(meeting.meetingId);
    if (isTwoPane) {
      return;
    }
    _pushMeeting(context, meeting.meetingId);
  }

  /// Starts a meeting with no recording (#129): asks for its title, creates it, and opens it to
  /// be filled by writing notes or adding files.
  ///
  /// Side effects: Creates the meeting through the core and reloads the list; a refusal shows
  /// a snack bar.
  Future<void> _newMeeting(MeetingEditController edits,
      {required bool isTwoPane}) async {
    final ScaffoldMessengerState? messenger =
        ScaffoldMessenger.maybeOf(context);
    final String? title = await showNewMeetingDialog(context);
    if (title == null || !mounted) {
      return;
    }
    final ({String? meetingId, String? error}) created =
        await edits.createMeeting(widget.controller, title: title);
    if (!mounted) {
      return;
    }
    switch (created) {
      case (meetingId: final String meetingId, error: _):
        widget.controller.selectMeeting(meetingId);
        if (!isTwoPane) {
          _pushMeeting(context, meetingId);
        }
      case (meetingId: null, error: final String? error):
        messenger?.showSnackBar(
            SnackBar(content: Text(error ?? l10n.somethingWentWrongTryAgain)));
    }
  }

  /// Opens the search page over every meeting; a result opens its meeting at the passage.
  void _search(SearchBridge bridge) {
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (BuildContext context) => SearchScreen(
        bridge: bridge,
        scope: SearchScope.meetings,
        title: l10n.searchMeetings,
        onOpen: (BuildContext context, SearchHit hit) {
          if (hit.meetingId case final String meetingId) {
            widget.controller.selectMeeting(meetingId);
            _pushMeeting(context, meetingId, focusStartMs: hit.startTimeMs);
          }
        },
      ),
    ));
  }

  /// Opens a meeting a chat answer cites, at the cited moment (#110).
  void _openCitedMeeting(BuildContext context, String meetingId, int? atMs) {
    widget.controller.selectMeeting(meetingId);
    _pushMeeting(context, meetingId, focusStartMs: atMs);
  }

  /// Pushes a meeting's page, scrolled to `focusStartMs` when given.
  void _pushMeeting(BuildContext context, String meetingId,
      {int? focusStartMs}) {
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (BuildContext context) => MeetingDetailScreen(
        controller: widget.controller,
        meetingId: meetingId,
        focusStartMs: focusStartMs,
        attachmentsController: widget.attachmentsController,
        photoScreenBuilder: widget.photoScreenBuilder,
        speakersScreenBuilder: widget.speakersScreenBuilder,
        exportController: widget.exportController,
        projectsController: widget.projectsController,
        summariesController: widget.summariesController,
        editController: widget.editController,
        speakerRenamer: widget.speakerRenamer,
        chatBridge: widget.chatBridge,
        recordingsBridge: widget.recordingsBridge,
      ),
    ));
  }

  @override
  Widget build(BuildContext context) {
    // "/" searches the meetings (#215); Escape and Back leave selection mode (#216).
    return PopScope(
      canPop: _selection.isEmpty,
      onPopInvokedWithResult: (bool didPop, Object? result) {
        if (!didPop) {
          _clearSelection();
        }
      },
      child: AppShortcuts(
        bindings: <ShortcutActivator, VoidCallback>{
          if (widget.searchBridge case final SearchBridge bridge)
            const CharacterActivator('/'): () => _search(bridge),
          if (_selection.isNotEmpty)
            const SingleActivator(LogicalKeyboardKey.escape): _clearSelection,
        },
        child: _layout(context),
      ),
    );
  }

  /// The list, and the shown meeting beside it on wide screens.
  Widget _layout(BuildContext context) {
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final bool isTwoPane = constraints.maxWidth >= twoPaneBreakpoint;
        final Widget meetingList = _MeetingList(
          controller: widget.controller,
          projectsController: widget.projectsController,
          summariesController: widget.summariesController,
          highlightSelection: isTwoPane,
          selection: _selection,
          onToggleSelected: _toggleSelected,
          onOpenMeeting: (MeetingSummary meeting) =>
              _openMeeting(meeting, isTwoPane: isTwoPane),
        );
        return Scaffold(
          appBar: _selection.isNotEmpty
              ? _selectionBar()
              : AppBar(
                  title: Text(l10n.meetings),
                  actions: <Widget>[
                    if (widget.editController
                        case final MeetingEditController edits)
                      IconButton(
                        key: const ValueKey<String>('new-meeting'),
                        tooltip: l10n.newMeeting,
                        icon: const Icon(Icons.note_add_outlined),
                        onPressed: () =>
                            _newMeeting(edits, isTwoPane: isTwoPane),
                      ),
                    if (widget.searchBridge case final SearchBridge bridge)
                      IconButton(
                        tooltip: l10n.searchMeetings,
                        icon: const Icon(Icons.search),
                        onPressed: () => _search(bridge),
                      ),
                    if (widget.chatBridge case final ChatBridge bridge)
                      IconButton(
                        tooltip: l10n.chatWithAllYourNotes,
                        icon: const Icon(Icons.forum_outlined),
                        onPressed: () => openChat(context,
                            bridge: bridge,
                            topic: const ChatTopic.everything(),
                            title: l10n.chatWithAllNotes,
                            openMeeting: _openCitedMeeting,
                            projectsController: widget.projectsController,
                            handoff: widget.summariesController?.handoff),
                      ),
                    IconButton(
                      tooltip: l10n.refresh,
                      icon: const Icon(Icons.refresh),
                      onPressed: widget.controller.loadMeetings,
                    ),
                  ],
                ),
          body: isTwoPane
              ? Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    ResizablePane(
                      id: 'meeting-list',
                      defaultWidth: _listPaneWidth,
                      maxWidth: constraints.maxWidth - _minDetailWidth,
                      child: meetingList,
                    ),
                    Expanded(
                        // The meeting's actions are in its own header (#201).
                        child: MeetingDetailView(
                            controller: widget.controller,
                            editController: widget.editController,
                            speakerRenamer: widget.speakerRenamer,
                            recordingsBridge: widget.recordingsBridge,
                            exportController: widget.exportController,
                            chatBridge: widget.chatBridge,
                            openCitedMeeting: _openCitedMeeting,
                            summariesController: widget.summariesController,
                            speakersScreenBuilder: widget.speakersScreenBuilder,
                            photoScreenBuilder: widget.photoScreenBuilder,
                            attachmentsController: widget.attachmentsController,
                            projectsController: widget.projectsController)),
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
    required this.selection,
    required this.onToggleSelected,
    this.projectsController,
    this.summariesController,
  });

  /// The meetings chosen for a batch action; while any is, a tap selects instead of opening.
  final Set<String> selection;
  final ValueChanged<MeetingSummary> onToggleSelected;

  final MeetingListController controller;

  /// Offers "Add to project…" in each row's menu when given (#94).
  final ProjectsController? projectsController;

  /// Adds the summary button and "Summary…" to each row when given (#95).
  final SummariesController? summariesController;

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
                  child: Text(l10n.retry),
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
                Text(l10n.noMeetingsYet, style: theme.textTheme.titleMedium),
                const SizedBox(height: 4),
                Text(l10n.recordingsYouStartAppearHere),
                const SizedBox(height: 16),
                // The next step, right where the list will be (#204).
                FilledButton.icon(
                  onPressed: () => HomeShell.openRecorder(context),
                  icon: const Icon(Icons.mic),
                  label: Text(l10n.recordAMeeting),
                ),
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
              final bool selecting = selection.isNotEmpty;
              final bool selected = selection.contains(meeting.meetingId);
              return _MeetingRow(
                controller: controller,
                projectsController: projectsController,
                summariesController: summariesController,
                meeting: meeting,
                onSelect: () => onToggleSelected(meeting),
                selecting: selecting,
                child: ListTile(
                  leading: selecting
                      ? Checkbox(
                          key: ValueKey<String>('select-${meeting.meetingId}'),
                          value: selected,
                          onChanged: (_) => onToggleSelected(meeting),
                        )
                      : const Icon(Icons.event_note),
                  selected: selected ||
                      (!selecting &&
                          highlightSelection &&
                          controller.selectedMeetingId == meeting.meetingId),
                  title: Text(
                    meeting.title,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.titleMedium
                        ?.copyWith(fontWeight: FontWeight.w600),
                  ),
                  subtitle: Text(
                    '${formatMeetingWhen(meeting.createdAt)} · '
                    '${formatMeetingDuration(meeting.durationMs)}'
                    '${meeting.recordedOnLabel == null ? '' : ' · ${meeting.recordedOnLabel}'}',
                  ),
                  trailing: selecting
                      ? null
                      : switch (summariesController) {
                          final SummariesController summaries =>
                            MeetingSummaryButton(
                              controller: summaries,
                              meetingId: meeting.meetingId,
                              title: meeting.title,
                            ),
                          null => null,
                        },
                  onTap: selecting
                      ? () => onToggleSelected(meeting)
                      : () => onOpenMeeting(meeting),
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

  /// Opens the project chooser (#94).
  addToProject,

  /// Opens the summary menu (#95).
  summary,

  /// Starts selecting meetings for a batch action (#216).
  select,

  /// Deletes the meeting after asking (#216: rows have no delete icon any more).
  delete,
}

/// Adds a context menu with "Rename" and, with projects, "Add to project…" to one meeting row.
///
/// The menu opens on long-press (touch), right-click (mouse), and Shift+F10 or the context menu
/// key (keyboard); screen readers get the same action as a custom semantics action.
final class _MeetingRow extends StatelessWidget {
  const _MeetingRow({
    required this.controller,
    required this.meeting,
    required this.child,
    required this.onSelect,
    required this.selecting,
    this.projectsController,
    this.summariesController,
  });

  final MeetingListController controller;
  final MeetingSummary meeting;

  /// Adds this meeting to the selection (#216).
  final VoidCallback onSelect;

  /// Whether a selection is being made: the row then has no swipe or hover actions.
  final bool selecting;

  /// Offers "Add to project…" when given.
  final ProjectsController? projectsController;

  /// Offers "Summary…" when given (#95).
  final SummariesController? summariesController;

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
      items: <PopupMenuEntry<_MeetingRowAction>>[
        PopupMenuItem<_MeetingRowAction>(
          key: const ValueKey<String>('row-select'),
          value: _MeetingRowAction.select,
          child: ListTile(
            leading: const Icon(Icons.check_box_outlined),
            title: Text(l10n.select),
          ),
        ),
        PopupMenuItem<_MeetingRowAction>(
          value: _MeetingRowAction.rename,
          child: ListTile(
            leading: Icon(Icons.edit_outlined),
            title: Text(l10n.rename),
          ),
        ),
        if (projectsController != null)
          PopupMenuItem<_MeetingRowAction>(
            value: _MeetingRowAction.addToProject,
            child: ListTile(
              leading: Icon(Icons.drive_file_move_outline),
              title: Text(l10n.addToProject),
            ),
          ),
        if (summariesController != null)
          PopupMenuItem<_MeetingRowAction>(
            value: _MeetingRowAction.summary,
            child: ListTile(
              leading: Icon(Icons.summarize_outlined),
              title: Text(l10n.summary2),
            ),
          ),
        const PopupMenuDivider(),
        PopupMenuItem<_MeetingRowAction>(
          key: const ValueKey<String>('row-delete'),
          value: _MeetingRowAction.delete,
          child: ListTile(
            leading: const Icon(Icons.delete_outline),
            title: Text(l10n.delete),
          ),
        ),
      ],
    );
    if (!context.mounted) {
      return;
    }
    switch (action) {
      case _MeetingRowAction.rename:
        await _rename(context);
      case _MeetingRowAction.addToProject:
        await _addToProject(context);
      case _MeetingRowAction.summary:
        await _summary(context);
      case _MeetingRowAction.select:
        onSelect();
      case _MeetingRowAction.delete:
        await deleteMeetingAfterAsking(context,
            controller: controller, meeting: meeting);
      case null:
        break;
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

  /// Opens the project chooser for this row's meeting (#94).
  Future<void> _addToProject(BuildContext context) async {
    if (projectsController case final ProjectsController projects) {
      await chooseProjectForMeeting(context,
          controller: projects, meetingId: meeting.meetingId);
    }
  }

  /// Opens the summary menu for this row's meeting (#95).
  Future<void> _summary(BuildContext context) async {
    if (summariesController case final SummariesController summaries) {
      await showMeetingSummaryMenu(context,
          controller: summaries, meeting: meeting);
    }
  }

  /// The row's quick delete (#216): a swipe to the start on phones and tablets, a button that
  /// appears on hover (or keyboard focus) on desktops, instead of a delete icon in every row.
  Widget _withRowActions(BuildContext context) {
    final TargetPlatform platform = Theme.of(context).platform;
    final bool touch =
        platform == TargetPlatform.android || platform == TargetPlatform.iOS;
    if (touch) {
      final ColorScheme colors = Theme.of(context).colorScheme;
      return Dismissible(
        key: ValueKey<String>('swipe-${meeting.meetingId}'),
        direction: DismissDirection.endToStart,
        background: ColoredBox(
          color: colors.errorContainer,
          child: Align(
            alignment: AlignmentDirectional.centerEnd,
            child: Padding(
              padding: const EdgeInsetsDirectional.only(end: 24),
              child: Icon(Icons.delete_outline, color: colors.onErrorContainer),
            ),
          ),
        ),
        // The list drops the row itself once the meeting is gone.
        confirmDismiss: (_) async {
          await deleteMeetingAfterAsking(context,
              controller: controller, meeting: meeting);
          return false;
        },
        child: child,
      );
    }
    return _HoverDelete(
      onDelete: () => deleteMeetingAfterAsking(context,
          controller: controller, meeting: meeting),
      deleteKey: ValueKey<String>('delete-${meeting.meetingId}'),
      child: child,
    );
  }

  @override
  Widget build(BuildContext context) {
    return Semantics(
      customSemanticsActions: <CustomSemanticsAction, VoidCallback>{
        CustomSemanticsAction(label: l10n.select): onSelect,
        CustomSemanticsAction(label: l10n.rename): () => _rename(context),
        if (projectsController != null)
          CustomSemanticsAction(label: l10n.addToProject2): () =>
              _addToProject(context),
        if (summariesController != null)
          CustomSemanticsAction(label: l10n.summary): () => _summary(context),
        CustomSemanticsAction(label: l10n.delete): () =>
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
          child: selecting ? child : _withRowActions(context),
        ),
      ),
    );
  }
}

/// The meeting list's app bar while selecting (#216), for tests.
const Key meetingSelectionBarKey = ValueKey<String>('meeting-selection-bar');

/// Shows a delete button over the end of a row while the pointer is over it or the keyboard focus
/// is inside it (#216), so rows stay uncluttered on desktops.
final class _HoverDelete extends StatefulWidget {
  const _HoverDelete(
      {required this.onDelete, required this.deleteKey, required this.child});

  final VoidCallback onDelete;
  final Key deleteKey;
  final Widget child;

  @override
  State<_HoverDelete> createState() => _HoverDeleteState();
}

final class _HoverDeleteState extends State<_HoverDelete> {
  bool _hovered = false;
  bool _focused = false;

  @override
  Widget build(BuildContext context) {
    final bool shown = _hovered || _focused;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: Focus(
        canRequestFocus: false,
        skipTraversal: true,
        onFocusChange: (bool focused) => setState(() => _focused = focused),
        child: Stack(
          alignment: AlignmentDirectional.centerEnd,
          children: <Widget>[
            widget.child,
            if (shown)
              Padding(
                padding: const EdgeInsetsDirectional.only(end: 8),
                child: IconButton.filledTonal(
                  key: widget.deleteKey,
                  tooltip: l10n.deleteMeeting,
                  icon: const Icon(Icons.delete_outline),
                  onPressed: widget.onDelete,
                ),
              ),
          ],
        ),
      ),
    );
  }
}
