// One project's page (Issue #85): its meetings, newest first, and its resources (files, notes
// and e-mails), with actions to add files, write a note, add meetings, rename and delete.

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/presentation/confirm_delete_dialog.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/meeting_summary_screen.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/presentation/resource_viewer_screen.dart';

/// Shows and changes one project.
final class ProjectDetailScreen extends StatelessWidget {
  /// Creates the page; the caller has asked the controller to open `projectId`.
  const ProjectDetailScreen({
    required this.controller,
    required this.projectId,
    this.meetingScreenBuilder,
    this.pdfViewBuilder = defaultPdfView,
    this.summariesController,
    super.key,
  });

  final ProjectsController controller;
  final String projectId;
  final MeetingScreenBuilder? meetingScreenBuilder;
  final PdfViewBuilder pdfViewBuilder;

  /// Adds a summary button and menu (long press, right-click) to each meeting when given (#95).
  final SummariesController? summariesController;

  /// Shows `message` in a snack bar.
  static void _tell(BuildContext context, String message) =>
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(message)));

  Future<void> _addFiles(BuildContext context) async {
    final String? message = await controller.addFiles(projectId);
    if (message != null && context.mounted) {
      _tell(context, message);
    }
  }

  Future<void> _addNote(BuildContext context) async {
    final (String, String)? note = await askNote(context);
    if (note == null) {
      return;
    }
    final bool saved = await controller.addNote(projectId, note.$1, note.$2);
    if (context.mounted) {
      _tell(context, saved ? 'Note saved' : 'The note could not be saved.');
    }
  }

  Future<void> _addMeetings(BuildContext context) async {
    final List<MeetingSummary> candidates =
        await controller.meetingsOutsideProjects();
    if (!context.mounted) {
      return;
    }
    final List<String> chosen = await pickMeetings(context, candidates);
    var failed = 0;
    for (final String meetingId in chosen) {
      if (!await controller.moveMeeting(meetingId, projectId)) {
        failed++;
      }
    }
    if (chosen.isNotEmpty && context.mounted) {
      _tell(
          context,
          failed == 0
              ? 'Added ${chosen.length} ${chosen.length == 1 ? 'meeting' : 'meetings'}'
              : '$failed could not be added.');
    }
  }

  Future<void> _rename(BuildContext context, ProjectSummary project) async {
    final String? name = await askProjectName(context,
        title: 'Rename project',
        confirmLabel: 'Rename',
        initialName: project.name);
    if (name == null || name == project.name) {
      return;
    }
    if (!await controller.renameProject(projectId, name) && context.mounted) {
      _tell(context, 'The project could not be renamed.');
    }
  }

  Future<void> _delete(BuildContext context, ProjectSummary project) async {
    final bool confirmed = await confirmDelete(
      context,
      title: 'Delete "${project.name}"?',
      message: 'Its files, notes and e-mails are deleted on every paired '
          'device. Its meetings are kept, outside any project.',
    );
    if (!confirmed || !context.mounted) {
      return;
    }
    if (await controller.deleteProject(projectId)) {
      if (context.mounted) {
        Navigator.of(context).pop();
      }
    } else if (context.mounted) {
      _tell(context, 'The project could not be deleted.');
    }
  }

  Future<void> _deleteResource(
      BuildContext context, ProjectResource resource) async {
    final bool confirmed = await confirmDelete(
      context,
      title: 'Delete "${resource.fileName}"?',
      message: 'It is deleted on every paired device.',
    );
    if (confirmed &&
        !await controller.deleteResource(projectId, resource.attachmentId) &&
        context.mounted) {
      _tell(context, 'The file could not be deleted.');
    }
  }

  Future<void> _removeMeeting(
      BuildContext context, MeetingSummary meeting) async {
    if (!await controller.moveMeeting(meeting.meetingId, null) &&
        context.mounted) {
      _tell(context, 'The meeting could not be removed from the project.');
    }
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (BuildContext context, Widget? child) {
        final ProjectDetail? detail = controller.openProjectId == projectId
            ? controller.openDetail
            : null;
        final ProjectSummary? project = detail?.project;
        return Scaffold(
          appBar: AppBar(
            title: Text(project?.name ?? 'Project'),
            actions: <Widget>[
              IconButton(
                key: const ValueKey<String>('add-project-files'),
                tooltip: 'Add files',
                icon: const Icon(Icons.attach_file),
                onPressed: controller.isBusy || detail == null
                    ? null
                    : () => _addFiles(context),
              ),
              IconButton(
                key: const ValueKey<String>('add-project-note'),
                tooltip: 'Write a note',
                icon: const Icon(Icons.note_add_outlined),
                onPressed: controller.isBusy || detail == null
                    ? null
                    : () => _addNote(context),
              ),
              IconButton(
                key: const ValueKey<String>('add-project-meetings'),
                tooltip: 'Add meetings',
                icon: const Icon(Icons.playlist_add),
                onPressed: detail == null ? null : () => _addMeetings(context),
              ),
              if (project != null)
                PopupMenuButton<String>(
                  key: const ValueKey<String>('project-menu'),
                  onSelected: (String action) => switch (action) {
                    'rename' => _rename(context, project),
                    _ => _delete(context, project),
                  },
                  itemBuilder: (BuildContext context) =>
                      const <PopupMenuEntry<String>>[
                    PopupMenuItem<String>(
                        value: 'rename', child: Text('Rename')),
                    PopupMenuItem<String>(
                        value: 'delete', child: Text('Delete project')),
                  ],
                ),
            ],
          ),
          body: switch (detail) {
            null when controller.detailErrorMessage != null =>
              Center(child: Text(controller.detailErrorMessage!)),
            null => const Center(child: CircularProgressIndicator()),
            final ProjectDetail detail =>
              _ProjectBody(screen: this, detail: detail),
          },
        );
      },
    );
  }
}

/// The meetings and resources of a loaded project.
final class _ProjectBody extends StatelessWidget {
  const _ProjectBody({required this.screen, required this.detail});

  final ProjectDetailScreen screen;
  final ProjectDetail detail;

  /// Opens the meeting's summary menu on long press or right-click, when summaries are offered.
  Widget _withSummaryMenu(
      BuildContext context, MeetingSummary meeting, Widget row) {
    final SummariesController? summaries = screen.summariesController;
    if (summaries == null) {
      return row;
    }
    void open() => showMeetingSummaryMenu(context,
        controller: summaries, meeting: meeting);
    return Semantics(
      customSemanticsActions: <CustomSemanticsAction, VoidCallback>{
        const CustomSemanticsAction(label: 'Summary'): open,
      },
      child: GestureDetector(
        onLongPress: open,
        onSecondaryTap: open,
        child: row,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final MaterialLocalizations localizations =
        MaterialLocalizations.of(context);
    Widget header(String text) => Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 4),
          child: Text(text,
              style: theme.textTheme.titleSmall
                  ?.copyWith(color: theme.colorScheme.primary)),
        );
    return RefreshIndicator(
      onRefresh: screen.controller.refresh,
      child: ListView(
        children: <Widget>[
          header('Meetings'),
          if (detail.meetings.isEmpty)
            const ListTile(
              title: Text('No meetings yet'),
              subtitle: Text('Add them here, or from a meeting\'s page.'),
            ),
          for (final MeetingSummary meeting in detail.meetings)
            _withSummaryMenu(
              context,
              meeting,
              ListTile(
                key: ValueKey<String>('project-meeting-${meeting.meetingId}'),
                leading: const Icon(Icons.event_note_outlined),
                title: Text(meeting.title),
                subtitle: Text(localizations
                    .formatMediumDate(meeting.createdAt.toLocal())),
                onTap: switch (screen.meetingScreenBuilder) {
                  // On return, the meeting may have moved to another project.
                  final MeetingScreenBuilder builder => () =>
                      Navigator.of(context)
                          .push(MaterialPageRoute<void>(
                            builder: (BuildContext context) =>
                                builder(context, meeting.meetingId),
                          ))
                          .then((_) => screen.controller.refresh()),
                  null => null,
                },
                trailing: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    if (screen.summariesController
                        case final SummariesController summaries)
                      MeetingSummaryButton(
                        controller: summaries,
                        meetingId: meeting.meetingId,
                        title: meeting.title,
                      ),
                    IconButton(
                      tooltip: 'Remove from project',
                      icon: const Icon(Icons.remove_circle_outline),
                      onPressed: () => screen._removeMeeting(context, meeting),
                    ),
                  ],
                ),
              ),
            ),
          header('Files, notes and e-mails'),
          if (detail.resources.isEmpty)
            const ListTile(
              title: Text('Nothing added yet'),
              subtitle: Text(
                  'Add pictures, Markdown, PDFs, e-mails (.eml) or documents.'),
            ),
          for (final ProjectResource resource in detail.resources)
            ListTile(
              key:
                  ValueKey<String>('project-resource-${resource.attachmentId}'),
              leading: Icon(_resourceIcon(resource.kind)),
              title: Text(resource.fileName),
              subtitle: Text(resource.isAvailable
                  ? '${localizations.formatMediumDate(resource.addedAt.toLocal())}'
                      ' · ${_size(resource.sizeBytes ?? 0)}'
                  : 'On its way from a paired device'),
              onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
                builder: (BuildContext context) => ResourceViewerScreen(
                  controller: screen.controller,
                  projectId: screen.projectId,
                  resource: resource,
                  pdfViewBuilder: screen.pdfViewBuilder,
                ),
              )),
              trailing: IconButton(
                tooltip: 'Delete',
                icon: const Icon(Icons.delete_outline),
                onPressed: () => screen._deleteResource(context, resource),
              ),
            ),
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}

/// The icon of a resource kind.
IconData _resourceIcon(ProjectResourceKind kind) => switch (kind) {
      ProjectResourceKind.image => Icons.image_outlined,
      ProjectResourceKind.markdown => Icons.notes,
      ProjectResourceKind.pdf => Icons.picture_as_pdf_outlined,
      ProjectResourceKind.email => Icons.mail_outline,
      ProjectResourceKind.text => Icons.text_snippet_outlined,
      ProjectResourceKind.document => Icons.description_outlined,
    };

/// A readable file size.
String _size(int bytes) {
  if (bytes < 1024) {
    return '$bytes B';
  }
  if (bytes < 1024 * 1024) {
    return '${(bytes / 1024).toStringAsFixed(0)} KB';
  }
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}
