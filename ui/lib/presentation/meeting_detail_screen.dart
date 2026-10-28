// Renders one meeting with its title as the header, followed by metadata and transcript.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/rename_meeting_dialog.dart';

/// Widest the detail content grows, in logical pixels, so transcript lines stay readable.
const double _maxReadableWidth = 840;

/// Shows one meeting as a full screen; used on phones, where the list pushes it as a route.
final class MeetingDetailScreen extends StatefulWidget {
  /// Creates a screen that selects and renders `meetingId` through `controller`.
  ///
  /// Inputs: The shared meeting controller, the meeting to show, and an optional builder for
  /// the attachment import screen (the paperclip action is hidden without one).
  /// Outputs: A Material 3 detail screen.
  /// Side effects: Loads the meeting through the controller after the first frame, unless the
  /// caller already selected it (the list does, so the route never shows another meeting).
  const MeetingDetailScreen({
    required this.controller,
    required this.meetingId,
    this.attachmentScreenBuilder,
    super.key,
  });

  final MeetingListController controller;
  final String meetingId;
  final WidgetBuilder? attachmentScreenBuilder;

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
          if (widget.attachmentScreenBuilder case final WidgetBuilder builder)
            AttachFileButton(attachmentScreenBuilder: builder),
        ],
      ),
      body: MeetingDetailView(controller: widget.controller),
    );
  }
}

/// Opens the attachment import screen from a meeting's app bar.
final class AttachFileButton extends StatelessWidget {
  /// Creates the paperclip action pushing `attachmentScreenBuilder`.
  const AttachFileButton({required this.attachmentScreenBuilder, super.key});

  final WidgetBuilder attachmentScreenBuilder;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      tooltip: 'Attach file',
      icon: const Icon(Icons.attach_file),
      onPressed: () => Navigator.of(context)
          .push(MaterialPageRoute<void>(builder: attachmentScreenBuilder)),
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

  /// Builds one block per segment, the flat transcript, or an empty-transcript message.
  List<Widget> _transcriptChildren(ThemeData theme) {
    if (meeting.segments.isNotEmpty) {
      return meeting.segments
          .map((MeetingTranscriptSegment segment) => Padding(
                padding: const EdgeInsets.only(bottom: 16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      '${segment.speaker} · '
                      '${formatMeetingDuration(segment.startTimeMs)}',
                      style: theme.textTheme.labelLarge
                          ?.copyWith(color: theme.colorScheme.primary),
                    ),
                    const SizedBox(height: 4),
                    Text(segment.text, style: theme.textTheme.bodyLarge),
                  ],
                ),
              ))
          .toList(growable: false);
    }
    if (meeting.transcript case final String transcript
        when transcript.trim().isNotEmpty) {
      return <Widget>[Text(transcript, style: theme.textTheme.bodyLarge)];
    }
    return <Widget>[
      Text(
        'No transcript is available.',
        style: theme.textTheme.bodyLarge
            ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
      ),
    ];
  }
}
