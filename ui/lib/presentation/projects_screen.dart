// The Projects tab (Issue #85): folders of meetings and resources, nested to any depth (#142).
// Creating a project asks for its name. Search (#100) finds words in every project's files and
// meetings.
//
// Phones list the top-level projects and open each as a page, whose subprojects open as further
// pages. Wide windows (#142) show columns, each resizable: the project tree, then the chosen
// project's subprojects, meetings and files (its page), then the chosen meeting or file.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/infrastructure/search_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/pdf_view.dart';
import 'package:note_taker_ui/presentation/project_detail_screen.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart'
    show twoPaneBreakpoint;
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/presentation/resizable_pane.dart';
import 'package:note_taker_ui/presentation/resource_viewer_screen.dart';
import 'package:note_taker_ui/presentation/search_screen.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

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

/// Requests the list once, and holds what the desktop columns show.
final class _ProjectsScreenState extends State<ProjectsScreen> {
  /// Whether the last layout had columns.
  bool _isWide = false;

  /// The project whose page is the middle column (desktop).
  String? _selectedProjectId;

  /// Projects whose subprojects the tree shows (desktop).
  final Set<String> _expanded = <String>{};

  /// The meeting shown in the last column (desktop).
  String? _shownMeetingId;

  /// The file, note or e-mail shown in the last column (desktop).
  ProjectResource? _shownResource;

  @override
  void initState() {
    super.initState();
    widget.controller.loadProjects();
  }

  /// Chooses the project of the middle column, showing it and its parents open in the tree.
  void _select(String projectId) {
    widget.controller.openProject(projectId);
    setState(() {
      _selectedProjectId = projectId;
      _shownMeetingId = null;
      _shownResource = null;
      String? current = projectId;
      for (int step = 0;
          current != null && step <= widget.controller.projects.length;
          step++) {
        _expanded.add(current);
        current = widget.controller.projectById(current)?.parentId;
      }
    });
  }

  /// Opens a project: in the columns on wide windows, as a page on phones.
  void _open(String projectId) {
    if (_isWide) {
      _select(projectId);
      return;
    }
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
        title: l10n.searchProjects,
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
        title: l10n.newProject2, confirmLabel: l10n.create);
    if (name == null || !mounted) {
      return;
    }
    final ProjectSummary? created = await widget.controller.createProject(name);
    if (!mounted) {
      return;
    }
    if (created == null) {
      ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.theProjectCouldNotBeCreated)));
      return;
    }
    _open(created.projectId);
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        _isWide = constraints.maxWidth >= twoPaneBreakpoint;
        return _isWide
            ? _columns(context, constraints.maxWidth)
            : _phone(context);
      },
    );
  }

  /// The search and refresh actions of both layouts.
  List<Widget> _commonActions() => <Widget>[
        if (widget.searchBridge case final SearchBridge bridge)
          IconButton(
            tooltip: l10n.searchProjects,
            icon: const Icon(Icons.search),
            onPressed: () => _search(bridge),
          ),
        IconButton(
          tooltip: l10n.refresh,
          icon: const Icon(Icons.refresh),
          onPressed: widget.controller.refresh,
        ),
      ];

  /// The desktop columns: tree, project page, meeting or file (#142).
  Widget _columns(BuildContext context, double width) {
    final double paneLimit = (width - _minShownWidth) / 2;
    final String? selected = _selectedProjectId;
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.projects),
        actions: <Widget>[
          ..._commonActions(),
          IconButton(
            key: const ValueKey<String>('new-project'),
            tooltip: l10n.newProject2,
            icon: const Icon(Icons.create_new_folder_outlined),
            onPressed: _create,
          ),
        ],
      ),
      // Before any project exists, one empty state with the button instead of three empty
      // columns (#204).
      body: AnimatedBuilder(
        animation: widget.controller,
        builder: (BuildContext context, Widget? columns) =>
            !widget.controller.isLoading &&
                    widget.controller.errorMessage == null &&
                    widget.controller.projects.isEmpty
                ? _NoProjects(onCreate: _create)
                : columns!,
        child: _columnsBody(context, paneLimit, selected),
      ),
    );
  }

  /// The three columns of the desktop layout.
  Widget _columnsBody(
      BuildContext context, double paneLimit, String? selected) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ResizablePane(
          id: 'projects-tree',
          defaultWidth: 260,
          maxWidth: paneLimit,
          child: AnimatedBuilder(
            animation: widget.controller,
            builder: (BuildContext context, Widget? child) => _ProjectTree(
              controller: widget.controller,
              selectedProjectId: selected,
              expanded: _expanded,
              onSelect: _select,
              onToggle: (String projectId) => setState(() {
                if (!_expanded.remove(projectId)) {
                  _expanded.add(projectId);
                }
              }),
            ),
          ),
        ),
        ResizablePane(
          id: 'project-contents',
          defaultWidth: 360,
          minWidth: 300,
          maxWidth: paneLimit,
          child: selected == null
              ? _Hint(text: l10n.chooseAProject)
              : ProjectDetailScreen(
                  key: ValueKey<String>('project-page-$selected'),
                  controller: widget.controller,
                  projectId: selected,
                  pdfViewBuilder: widget.pdfViewBuilder,
                  summariesController: widget.summariesController,
                  searchBridge: widget.searchBridge,
                  chatBridge: widget.chatBridge,
                  meetingMomentBuilder: widget.searchMeetingBuilder,
                  meetingScreenBuilder: widget.meetingScreenBuilder,
                  selectedItemId:
                      _shownMeetingId ?? _shownResource?.attachmentId,
                  onOpenMeeting: widget.meetingScreenBuilder == null
                      ? null
                      : (String meetingId) => setState(() {
                            _shownMeetingId = meetingId;
                            _shownResource = null;
                          }),
                  onOpenResource: (ProjectResource resource) => setState(() {
                    _shownResource = resource;
                    _shownMeetingId = null;
                  }),
                  onOpenProject: _select,
                  onDeleted: () => setState(() {
                    _selectedProjectId = null;
                    _shownMeetingId = null;
                    _shownResource = null;
                  }),
                ),
        ),
        Expanded(child: _shown(context, selected)),
      ],
    );
  }

  /// The last column: the chosen meeting's page, the chosen file, or a hint.
  Widget _shown(BuildContext context, String? projectId) {
    if ((_shownMeetingId, widget.meetingScreenBuilder)
        case (final String meetingId, final MeetingScreenBuilder builder)) {
      return KeyedSubtree(
        key: ValueKey<String>('shown-meeting-$meetingId'),
        child: Builder(
            builder: (BuildContext context) => builder(context, meetingId)),
      );
    }
    if ((_shownResource, projectId)
        case (final ProjectResource resource, final String projectId)) {
      return ResourceViewerScreen(
        key: ValueKey<String>('shown-resource-${resource.attachmentId}'),
        controller: widget.controller,
        projectId: projectId,
        resource: resource,
        pdfViewBuilder: widget.pdfViewBuilder,
      );
    }
    return _Hint(text: l10n.chooseAMeetingOrFile);
  }

  /// The phone list of top-level projects.
  Widget _phone(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.projects),
        actions: <Widget>[
          ..._commonActions(),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        key: const ValueKey<String>('new-project'),
        onPressed: _create,
        icon: const Icon(Icons.create_new_folder_outlined),
        label: Text(l10n.newProject2),
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
            return Center(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: Text(
                  l10n.noProjectsYetAProjectGathers,
                  textAlign: TextAlign.center,
                ),
              ),
            );
          }
          return ListView(
            padding: const EdgeInsets.only(bottom: 88),
            children: <Widget>[
              for (final ProjectSummary project in controller.childrenOf(null))
                ListTile(
                  key: ValueKey<String>('project-${project.projectId}'),
                  leading: const Icon(Icons.folder_outlined),
                  title: Text(project.name),
                  subtitle: Text(projectCounts(controller, project)),
                  onTap: () => _open(project.projectId),
                ),
            ],
          );
        },
      ),
    );
  }
}

/// Room always left to the meeting or file beside the two project columns.
const double _minShownWidth = 400;

/// How far each tree level is indented.
const double _treeIndent = 16;

/// The desktop page before any project exists (#204): what a project is, and the button that
/// makes the first one.
final class _NoProjects extends StatelessWidget {
  const _NoProjects({required this.onCreate});

  final VoidCallback onCreate;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 420),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(Icons.folder_outlined,
                size: 48, color: theme.colorScheme.onSurfaceVariant),
            const SizedBox(height: 12),
            Text(l10n.noProjectsYetAProjectGathers,
                textAlign: TextAlign.center),
            const SizedBox(height: 16),
            FilledButton.icon(
              onPressed: onCreate,
              icon: const Icon(Icons.create_new_folder_outlined),
              label: Text(l10n.newProject2),
            ),
          ],
        ),
      ),
    );
  }
}

/// A centred hint in an empty column.
final class _Hint extends StatelessWidget {
  const _Hint({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Text(text,
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant)),
        ),
      );
}

/// The desktop's project tree (#142): top-level projects, and the subprojects of open ones.
final class _ProjectTree extends StatelessWidget {
  const _ProjectTree({
    required this.controller,
    required this.selectedProjectId,
    required this.expanded,
    required this.onSelect,
    required this.onToggle,
  });

  final ProjectsController controller;
  final String? selectedProjectId;
  final Set<String> expanded;
  final void Function(String projectId) onSelect;
  final void Function(String projectId) onToggle;

  @override
  Widget build(BuildContext context) {
    if (controller.isLoading && controller.projects.isEmpty) {
      return const Center(child: CircularProgressIndicator());
    }
    if (controller.errorMessage case final String message) {
      return _Hint(text: message);
    }
    if (controller.projects.isEmpty) {
      return _Hint(text: l10n.noProjectsYetAProjectGathers);
    }
    final List<Widget> rows = <Widget>[];
    final Set<String> seen = <String>{};
    void add(String? parentId, int depth) {
      for (final ProjectSummary project in controller.childrenOf(parentId)) {
        if (!seen.add(project.projectId)) {
          continue;
        }
        final bool hasChildren =
            controller.childrenOf(project.projectId).isNotEmpty;
        final bool isOpen = expanded.contains(project.projectId);
        rows.add(ListTile(
          key: ValueKey<String>('tree-project-${project.projectId}'),
          dense: true,
          selected: project.projectId == selectedProjectId,
          contentPadding: EdgeInsetsDirectional.only(
              start: 4 + depth * _treeIndent, end: 8),
          leading: SizedBox(
            width: 64,
            child: Row(
              children: <Widget>[
                if (hasChildren)
                  IconButton(
                    visualDensity: VisualDensity.compact,
                    tooltip:
                        MaterialLocalizations.of(context).expandedIconTapHint,
                    icon:
                        Icon(isOpen ? Icons.expand_more : Icons.chevron_right),
                    onPressed: () => onToggle(project.projectId),
                  )
                else
                  const SizedBox(width: 40),
                Icon(isOpen && hasChildren
                    ? Icons.folder_open_outlined
                    : Icons.folder_outlined),
              ],
            ),
          ),
          title:
              Text(project.name, maxLines: 1, overflow: TextOverflow.ellipsis),
          onTap: () => onSelect(project.projectId),
        ));
        if (isOpen) {
          add(project.projectId, depth + 1);
        }
      }
    }

    add(null, 0);
    return ListView(
      padding: const EdgeInsets.symmetric(vertical: 8),
      children: rows,
    );
  }
}
