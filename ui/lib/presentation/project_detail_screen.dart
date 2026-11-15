// One project's page (Issue #85): its meetings, newest first, and its resources (files, notes
// and e-mails), with actions to add files, write a note, add meetings, rename and delete, and to
// search this project's files and meetings (#100).

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/infrastructure/search_bridge.dart';
import 'package:note_taker_ui/presentation/confirm_delete_dialog.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/meeting_summary_screen.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/presentation/resource_viewer_screen.dart';
import 'package:note_taker_ui/presentation/chat_screen.dart';
import 'package:note_taker_ui/presentation/search_screen.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Shows and changes one project.
final class ProjectDetailScreen extends StatelessWidget {
  /// Creates the page; the caller has asked the controller to open `projectId`.
  const ProjectDetailScreen({
    required this.controller,
    required this.projectId,
    this.meetingScreenBuilder,
    this.pdfViewBuilder = defaultPdfView,
    this.summariesController,
    this.searchBridge,
    this.chatBridge,
    this.meetingMomentBuilder,
    super.key,
  });

  final ProjectsController controller;
  final String projectId;
  final MeetingScreenBuilder? meetingScreenBuilder;
  final PdfViewBuilder pdfViewBuilder;

  /// Adds a summary button and menu (long press, right-click) to each meeting when given (#95).
  final SummariesController? summariesController;

  /// Adds Search over this project when given (#100).
  final SearchBridge? searchBridge;

  /// Adds Chat about this project when given (#97).
  final ChatBridge? chatBridge;

  /// Opens a meeting at a passage search found.
  final MeetingMomentBuilder? meetingMomentBuilder;

  /// Opens the search page over this project.
  void _search(BuildContext context, SearchBridge bridge, String name) {
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (BuildContext context) => SearchScreen(
        bridge: bridge,
        scope: SearchScope.project,
        projectId: projectId,
        title: l10n.search(name),
        onOpen: (BuildContext context, SearchHit hit) => openProjectSearchHit(
          context,
          hit,
          controller: controller,
          meetingBuilder: meetingMomentBuilder,
          projectPageBuilder: null,
        ),
      ),
    ));
  }

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
      _tell(context, saved ? l10n.noteSaved : l10n.theNoteCouldNotBeSaved);
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
              ? l10n.addedMeetings(chosen.length)
              : l10n.couldNotBeAdded2(failed));
    }
  }

  Future<void> _rename(BuildContext context, ProjectSummary project) async {
    final String? name = await askProjectName(context,
        title: l10n.renameProject,
        confirmLabel: l10n.rename,
        initialName: project.name);
    if (name == null || name == project.name) {
      return;
    }
    if (!await controller.renameProject(projectId, name) && context.mounted) {
      _tell(context, l10n.theProjectCouldNotBeRenamed);
    }
  }

  Future<void> _delete(BuildContext context, ProjectSummary project) async {
    final bool confirmed = await confirmDelete(
      context,
      title: l10n.delete3(project.name),
      message: l10n.itsFilesNotesAndEMails,
    );
    if (!confirmed || !context.mounted) {
      return;
    }
    if (await controller.deleteProject(projectId)) {
      if (context.mounted) {
        Navigator.of(context).pop();
      }
    } else if (context.mounted) {
      _tell(context, l10n.theProjectCouldNotBeDeleted);
    }
  }

  Future<void> _deleteResource(
      BuildContext context, ProjectResource resource) async {
    final bool confirmed = await confirmDelete(
      context,
      title: l10n.delete3(resource.fileName),
      message: l10n.itIsDeletedOnEveryPaired,
    );
    if (confirmed &&
        !await controller.deleteResource(projectId, resource.attachmentId) &&
        context.mounted) {
      _tell(context, l10n.theFileCouldNotBeDeleted);
    }
  }

  Future<void> _removeMeeting(
      BuildContext context, MeetingSummary meeting) async {
    if (!await controller.moveMeeting(meeting.meetingId, null) &&
        context.mounted) {
      _tell(context, l10n.theMeetingCouldNotBeRemoved);
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
            title: Text(project?.name ?? l10n.project),
            actions: <Widget>[
              if ((searchBridge, project)
                  case (
                    final SearchBridge bridge,
                    final ProjectSummary project
                  ))
                IconButton(
                  tooltip: l10n.searchThisProject,
                  icon: const Icon(Icons.search),
                  onPressed: () => _search(context, bridge, project.name),
                ),
              if ((chatBridge, project)
                  case (final ChatBridge bridge, final ProjectSummary project))
                IconButton(
                  tooltip: l10n.chatAboutThisProject,
                  icon: const Icon(Icons.chat_outlined),
                  onPressed: () => openChat(context,
                      bridge: bridge,
                      topic: ChatTopic.project(projectId),
                      title: l10n.chat(project.name),
                      openMeeting: switch (meetingMomentBuilder) {
                        final MeetingMomentBuilder builder =>
                          (BuildContext context, String meetingId, int? atMs) =>
                              Navigator.of(context)
                                  .push(MaterialPageRoute<void>(
                                builder: (BuildContext context) =>
                                    builder(context, meetingId, atMs),
                              )),
                        null => null,
                      },
                      projectsController: controller,
                      handoff: summariesController?.handoff),
                ),
              IconButton(
                key: const ValueKey<String>('add-project-files'),
                tooltip: l10n.addFiles,
                icon: const Icon(Icons.attach_file),
                onPressed: controller.isBusy || detail == null
                    ? null
                    : () => _addFiles(context),
              ),
              IconButton(
                key: const ValueKey<String>('add-project-note'),
                tooltip: l10n.writeANote,
                icon: const Icon(Icons.note_add_outlined),
                onPressed: controller.isBusy || detail == null
                    ? null
                    : () => _addNote(context),
              ),
              IconButton(
                key: const ValueKey<String>('add-project-meetings'),
                tooltip: l10n.addMeetings,
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
                      <PopupMenuEntry<String>>[
                    PopupMenuItem<String>(
                        value: 'rename', child: Text(l10n.rename)),
                    PopupMenuItem<String>(
                        value: 'delete', child: Text(l10n.deleteProject)),
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
        CustomSemanticsAction(label: l10n.summary): open,
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
          header(l10n.meetings),
          if (detail.meetings.isEmpty)
            ListTile(
              title: Text(l10n.noMeetingsYet),
              subtitle: Text(l10n.addThemHereOrFromA),
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
                      tooltip: l10n.removeFromProject,
                      icon: const Icon(Icons.remove_circle_outline),
                      onPressed: () => screen._removeMeeting(context, meeting),
                    ),
                  ],
                ),
              ),
            ),
          header(l10n.filesNotesAndEMails),
          if (detail.resources.isEmpty)
            ListTile(
              title: Text(l10n.nothingAddedYet),
              subtitle: Text(l10n.addPicturesMarkdownPdfsEMails),
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
                  : l10n.onItsWayFromAPaired),
              onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
                builder: (BuildContext context) => ResourceViewerScreen(
                  controller: screen.controller,
                  projectId: screen.projectId,
                  resource: resource,
                  pdfViewBuilder: screen.pdfViewBuilder,
                ),
              )),
              trailing: IconButton(
                tooltip: l10n.delete,
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
