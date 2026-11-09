// The Projects tab (Issue #85): folders of meetings and resources. Lists projects by name and
// opens each one; creating a project asks for its name. Search (#100) finds words in every
// project's files and meetings.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/infrastructure/search_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/project_detail_screen.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/presentation/resource_viewer_screen.dart';
import 'package:note_taker_ui/presentation/search_screen.dart';

/// Lists projects and opens them.
final class ProjectsScreen extends StatefulWidget {
  /// Creates the tab.
  ///
  /// Inputs: The controller, the builder of a meeting's page (to open a project's meetings), and
  /// optionally another PDF view for tests.
  /// Outputs: A Material 3 screen. Side effects: Loads the projects when first shown.
  const ProjectsScreen({
    required this.controller,
    this.meetingScreenBuilder,
    this.pdfViewBuilder = defaultPdfView,
    this.summariesController,
    this.searchBridge,
    this.chatBridge,
    this.meetingMomentBuilder,
    super.key,
  });

  final ProjectsController controller;
  final MeetingScreenBuilder? meetingScreenBuilder;
  final PdfViewBuilder pdfViewBuilder;

  /// Offers meeting summaries in the projects' meeting lists when given (#95).
  final SummariesController? summariesController;

  /// Adds Search to this tab and to each project's page when given (#100).
  final SearchBridge? searchBridge;

  /// Adds each project's chat when given (#97).
  final ChatBridge? chatBridge;

  /// Opens a meeting at a passage found by search; `meetingScreenBuilder` (at the top) otherwise.
  final MeetingMomentBuilder? meetingMomentBuilder;

  /// The builder search opens meetings with.
  MeetingMomentBuilder? get searchMeetingBuilder =>
      meetingMomentBuilder ??
      switch (meetingScreenBuilder) {
        final MeetingScreenBuilder builder =>
          (BuildContext context, String meetingId, int? startTimeMs) =>
              builder(context, meetingId),
        null => null,
      };

  @override
  State<ProjectsScreen> createState() => _ProjectsScreenState();
}

/// Requests the list once.
final class _ProjectsScreenState extends State<ProjectsScreen> {
  @override
  void initState() {
    super.initState();
    widget.controller.loadProjects();
  }

  /// Opens a project's page.
  void _open(String projectId) {
    widget.controller.openProject(projectId);
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (BuildContext context) => _projectPage(context, projectId),
    ));
  }

  /// Builds a project's page; the caller has asked the controller to open it.
  Widget _projectPage(BuildContext context, String projectId) =>
      ProjectDetailScreen(
        controller: widget.controller,
        projectId: projectId,
        meetingScreenBuilder: widget.meetingScreenBuilder,
        pdfViewBuilder: widget.pdfViewBuilder,
        summariesController: widget.summariesController,
        searchBridge: widget.searchBridge,
        chatBridge: widget.chatBridge,
        meetingMomentBuilder: widget.searchMeetingBuilder,
      );

  /// Opens the search page over every project.
  void _search(SearchBridge bridge) {
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (BuildContext context) => SearchScreen(
        bridge: bridge,
        scope: SearchScope.projects,
        title: 'Search projects',
        onOpen: (BuildContext context, SearchHit hit) => openProjectSearchHit(
          context,
          hit,
          controller: widget.controller,
          meetingBuilder: widget.searchMeetingBuilder,
          projectPageBuilder: _projectPage,
        ),
      ),
    ));
  }

  /// Asks for a name and creates the project, then opens it.
  Future<void> _create() async {
    final String? name = await askProjectName(context,
        title: 'New project', confirmLabel: 'Create');
    if (name == null || !mounted) {
      return;
    }
    final ProjectSummary? created = await widget.controller.createProject(name);
    if (!mounted) {
      return;
    }
    if (created == null) {
      ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('The project could not be created.')));
      return;
    }
    _open(created.projectId);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Projects'),
        actions: <Widget>[
          if (widget.searchBridge case final SearchBridge bridge)
            IconButton(
              tooltip: 'Search projects',
              icon: const Icon(Icons.search),
              onPressed: () => _search(bridge),
            ),
          IconButton(
            tooltip: 'Refresh',
            icon: const Icon(Icons.refresh),
            onPressed: widget.controller.loadProjects,
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        key: const ValueKey<String>('new-project'),
        onPressed: _create,
        icon: const Icon(Icons.create_new_folder_outlined),
        label: const Text('New project'),
      ),
      body: AnimatedBuilder(
        animation: widget.controller,
        builder: (BuildContext context, Widget? child) {
          final ProjectsController controller = widget.controller;
          if (controller.isLoading && controller.projects.isEmpty) {
            return const Center(child: CircularProgressIndicator());
          }
          if (controller.errorMessage case final String message) {
            return Center(child: Text(message));
          }
          if (controller.projects.isEmpty) {
            return const Center(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: Text(
                  'No projects yet. A project gathers meetings with the files, '
                  'notes and e-mails that belong to them.',
                  textAlign: TextAlign.center,
                ),
              ),
            );
          }
          return ListView(
            padding: const EdgeInsets.only(bottom: 88),
            children: <Widget>[
              for (final ProjectSummary project in controller.projects)
                ListTile(
                  key: ValueKey<String>('project-${project.projectId}'),
                  leading: const Icon(Icons.folder_outlined),
                  title: Text(project.name),
                  subtitle: Text(_counts(project)),
                  onTap: () => _open(project.projectId),
                ),
            ],
          );
        },
      ),
    );
  }
}

/// `2 meetings · 3 files`.
String _counts(ProjectSummary project) {
  String count(int value, String one, String many) =>
      '$value ${value == 1 ? one : many}';
  return '${count(project.meetingCount, 'meeting', 'meetings')} · '
      '${count(project.resourceCount, 'file', 'files')}';
}
