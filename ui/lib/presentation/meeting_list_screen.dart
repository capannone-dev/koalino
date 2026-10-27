// Renders the meeting list title-first, as a phone list or a desktop list-detail layout.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';

/// Window width from which the list and the detail are shown side by side (Material 3
/// "expanded" window class).
const double twoPaneBreakpoint = 840;

/// Width of the list pane in the two-pane layout.
const double _listPaneWidth = 360;

/// Lists meetings and opens their detail: as a route on phones, beside the list on desktops.
final class MeetingListScreen extends StatefulWidget {
  /// Creates a screen observing `controller`.
  ///
  /// Inputs: The meeting controller and an optional builder for the attachment import screen,
  /// passed on to the detail.
  /// Outputs: A Material 3 meeting list screen.
  /// Side effects: Loads the meetings when first inserted.
  const MeetingListScreen({
    required this.controller,
    this.attachmentScreenBuilder,
    super.key,
  });

  final MeetingListController controller;
  final WidgetBuilder? attachmentScreenBuilder;

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
        attachmentScreenBuilder: widget.attachmentScreenBuilder,
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
                if (widget.attachmentScreenBuilder
                    case final WidgetBuilder builder)
                  AnimatedBuilder(
                    animation: widget.controller,
                    builder: (BuildContext context, Widget? child) => widget
                                .controller.selectedMeeting ==
                            null
                        ? const SizedBox.shrink()
                        : AttachFileButton(attachmentScreenBuilder: builder),
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
              return ListTile(
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
                  '${formatMeetingDuration(meeting.durationMs)}',
                ),
                onTap: () => onOpenMeeting(meeting),
              );
            },
          ),
        );
      },
    );
  }
}
