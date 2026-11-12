// Shares from other apps (Issue #85): when a mail app, file manager or gallery shares to Note
// Taker, a sheet asks which project the files go into, or creates a new one. The copies the
// platform made are released afterwards, whether the files were added or the sheet dismissed.
// While a summary hand-off waits for its answer (#114), shared text is first offered as that
// meeting's summary.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/handoff_controller.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/incoming_share_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';

/// What the sheet answers: an existing project, or a request for a new one.
sealed class _ShareTarget {
  const _ShareTarget();
}

final class _ExistingProject extends _ShareTarget {
  const _ExistingProject(this.projectId);
  final String projectId;
}

final class _NewProject extends _ShareTarget {
  const _NewProject();
}

/// Offers each incoming share to a project, around the app's screens.
final class IncomingShareListener extends StatefulWidget {
  /// Creates the listener.
  ///
  /// Inputs: The share bridge, the projects controller that adds the files, the hand-off
  /// controller whose waiting summary a text share may answer (null where there is none), and
  /// the screens it wraps (it must sit below the app's navigator and `ScaffoldMessenger`).
  /// Outputs: `child`, unchanged. Side effects: Takes any share pending at start-up and each
  /// one that arrives later, and shows the sheet for it.
  const IncomingShareListener({
    required this.shareBridge,
    required this.projectsController,
    required this.child,
    this.handoffController,
    super.key,
  });

  final IncomingShareBridge shareBridge;

  /// Keeps a shared answer as the waiting summary; null without hand-offs.
  final HandoffController? handoffController;

  final ProjectsController projectsController;
  final Widget child;

  @override
  State<IncomingShareListener> createState() => _IncomingShareListenerState();
}

/// Handles one share at a time; a share arriving meanwhile waits as Kotlin's pending one.
final class _IncomingShareListenerState extends State<IncomingShareListener> {
  StreamSubscription<void>? _arrivals;
  bool _isHandling = false;

  @override
  void initState() {
    super.initState();
    _arrivals = widget.shareBridge.arrivals.listen((_) => _takeShare());
    // After the first frame, so the sheet has a navigator to show on.
    WidgetsBinding.instance.addPostFrameCallback((_) => _takeShare());
  }

  @override
  void dispose() {
    _arrivals?.cancel();
    super.dispose();
  }

  /// Takes the pending share, if any, and handles it; then looks for one that came meanwhile.
  Future<void> _takeShare() async {
    if (_isHandling || !mounted) {
      return;
    }
    _isHandling = true;
    try {
      IncomingShare? share = await widget.shareBridge.takePending();
      while (share != null && mounted) {
        await _handle(share);
        share = await widget.shareBridge.takePending();
      }
    } finally {
      _isHandling = false;
    }
  }

  /// Asks for the project, adds the files to it, reports the result, and releases the copies.
  Future<void> _handle(IncomingShare share) async {
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    try {
      if (share.filePaths.isEmpty) {
        messenger.showSnackBar(const SnackBar(
            content: Text('Nothing that was shared could be read.')));
        return;
      }
      if (await _keptAsSummary(share) || !mounted) {
        return;
      }
      final ProjectsController controller = widget.projectsController;
      await controller.loadProjects();
      if (!mounted) {
        return;
      }
      final _ShareTarget? target = await showModalBottomSheet<_ShareTarget>(
        context: context,
        showDragHandle: true,
        isScrollControlled: true,
        builder: (BuildContext context) => _ChooseProjectSheet(
          projects: controller.projects,
          fileCount: share.filePaths.length,
        ),
      );
      if (target == null || !mounted) {
        return;
      }
      final String? projectId = switch (target) {
        _ExistingProject(:final String projectId) => projectId,
        _NewProject() => await _createProject(),
      };
      if (projectId == null) {
        return;
      }
      String message = await controller.addPaths(projectId, share.filePaths);
      if (share.failedCount > 0) {
        message = '$message; ${share.failedCount} could not be read';
      }
      messenger.showSnackBar(SnackBar(content: Text(message)));
    } finally {
      await widget.shareBridge.release(share);
    }
  }

  /// Offers a text share as the summary a hand-off waits for.
  ///
  /// Outputs: true when handled: saved as the summary (the hand-off controller then tells the
  /// owner, with Undo) or the choice dismissed; false to add it to a project as usual. Side effects: May write the
  /// meeting's summary.
  Future<bool> _keptAsSummary(IncomingShare share) async {
    final HandoffController? handoff = widget.handoffController;
    final PendingAnswer? waiting = handoff?.pending;
    final String? text = share.sharedText;
    if (handoff == null ||
        waiting == null ||
        text == null ||
        text.trim().length < minAnswerCharacters) {
      return false;
    }
    final bool? asSummary = await showModalBottomSheet<bool>(
      context: context,
      showDragHandle: true,
      builder: (BuildContext context) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: <Widget>[
            ListTile(
              leading: const Icon(Icons.summarize_outlined),
              title: Text('Save as the summary of "${waiting.title}"'),
              onTap: () => Navigator.of(context).pop(true),
            ),
            ListTile(
              leading: const Icon(Icons.folder_outlined),
              title: const Text('Add to a project instead'),
              onTap: () => Navigator.of(context).pop(false),
            ),
          ],
        ),
      ),
    );
    if (asSummary != true) {
      return asSummary == null;
    }
    return handoff.keepSharedAnswer(text);
  }

  /// Asks for a name and creates the project; `null` when cancelled or refused.
  Future<String?> _createProject() async {
    final String? name = await askProjectName(context,
        title: 'New project', confirmLabel: 'Create');
    if (name == null || !mounted) {
      return null;
    }
    final ProjectSummary? created =
        await widget.projectsController.createProject(name);
    if (created == null && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('The project could not be created.')));
    }
    return created?.projectId;
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

/// Lists the projects to add the shared files to, with New project first.
final class _ChooseProjectSheet extends StatelessWidget {
  const _ChooseProjectSheet({required this.projects, required this.fileCount});

  final List<ProjectSummary> projects;
  final int fileCount;

  @override
  Widget build(BuildContext context) {
    final String heading = fileCount == 1
        ? 'Add the shared file to a project'
        : 'Add $fileCount shared files to a project';
    return SafeArea(
      child: ConstrainedBox(
        constraints:
            BoxConstraints(maxHeight: MediaQuery.sizeOf(context).height * 0.7),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 0, 24, 8),
              child: Semantics(
                header: true,
                child: Text(heading,
                    style: Theme.of(context).textTheme.titleLarge),
              ),
            ),
            ListTile(
              leading: const Icon(Icons.create_new_folder_outlined),
              title: const Text('New project'),
              onTap: () => Navigator.of(context).pop(const _NewProject()),
            ),
            const Divider(height: 1),
            Flexible(
              child: ListView(
                shrinkWrap: true,
                children: <Widget>[
                  for (final ProjectSummary project in projects)
                    ListTile(
                      leading: const Icon(Icons.folder_outlined),
                      title: Text(project.name),
                      onTap: () => Navigator.of(context)
                          .pop(_ExistingProject(project.projectId)),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
