// The Projects tab (Issue #85): folders of meetings and resources. Lists projects by name and
// opens each one; creating a project asks for its name.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/project_detail_screen.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/presentation/resource_viewer_screen.dart';

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
    super.key,
  });

  final ProjectsController controller;
  final MeetingScreenBuilder? meetingScreenBuilder;
  final PdfViewBuilder pdfViewBuilder;

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
      builder: (BuildContext context) => ProjectDetailScreen(
        controller: widget.controller,
        projectId: projectId,
        meetingScreenBuilder: widget.meetingScreenBuilder,
        pdfViewBuilder: widget.pdfViewBuilder,
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
