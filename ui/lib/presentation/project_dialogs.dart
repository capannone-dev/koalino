// Dialogs of the Projects tab (Issue #85): naming a project, writing a note, choosing meetings
// to add, and choosing the project a meeting belongs to.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Asks for a project name, checked as Rust checks it.
///
/// Inputs: A mounted context, the dialog's title and button label, and the current name.
/// Outputs: The trimmed name, or `null` when cancelled. Side effects: Shows a dialog.
Future<String?> askProjectName(
  BuildContext context, {
  required String title,
  required String confirmLabel,
  String initialName = '',
}) =>
    showDialog<String>(
      context: context,
      builder: (BuildContext context) => _ProjectNameDialog(
        title: title,
        confirmLabel: confirmLabel,
        initialName: initialName,
      ),
    );

/// The name field with live validation.
final class _ProjectNameDialog extends StatefulWidget {
  const _ProjectNameDialog({
    required this.title,
    required this.confirmLabel,
    required this.initialName,
  });

  final String title;
  final String confirmLabel;
  final String initialName;

  @override
  State<_ProjectNameDialog> createState() => _ProjectNameDialogState();
}

/// Holds the typed name.
final class _ProjectNameDialogState extends State<_ProjectNameDialog> {
  late final TextEditingController _name =
      TextEditingController(text: widget.initialName);
  bool _showError = false;

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  void _submit() {
    if (projectNameError(_name.text) != null) {
      setState(() => _showError = true);
      return;
    }
    Navigator.of(context).pop(_name.text.trim());
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
        title: Text(widget.title),
        content: TextField(
          key: const ValueKey<String>('project-name-field'),
          controller: _name,
          autofocus: true,
          decoration: InputDecoration(
            labelText: l10n.name,
            errorText: _showError ? projectNameError(_name.text) : null,
          ),
          onChanged: (_) {
            if (_showError) {
              setState(() {});
            }
          },
          onSubmitted: (_) => _submit(),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(l10n.cancel),
          ),
          FilledButton(onPressed: _submit, child: Text(widget.confirmLabel)),
        ],
      );
}

/// Asks for a note's title and text.
///
/// Outputs: `(title, text)`, or `null` when cancelled. Side effects: Shows a dialog.
Future<(String, String)?> askNote(BuildContext context) =>
    showDialog<(String, String)>(
      context: context,
      builder: (BuildContext context) => const _NoteDialog(),
    );

/// The note fields.
final class _NoteDialog extends StatefulWidget {
  const _NoteDialog();

  @override
  State<_NoteDialog> createState() => _NoteDialogState();
}

/// Holds the typed note.
final class _NoteDialogState extends State<_NoteDialog> {
  final TextEditingController _title = TextEditingController();
  final TextEditingController _text = TextEditingController();
  bool _showError = false;

  @override
  void dispose() {
    _title.dispose();
    _text.dispose();
    super.dispose();
  }

  void _submit() {
    if (noteTitleError(_title.text) != null) {
      setState(() => _showError = true);
      return;
    }
    Navigator.of(context).pop((_title.text.trim(), _text.text));
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
        title: Text(l10n.newNote),
        content: SizedBox(
          width: 480,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              TextField(
                key: const ValueKey<String>('note-title-field'),
                controller: _title,
                autofocus: true,
                decoration: InputDecoration(
                  labelText: l10n.title,
                  errorText: _showError ? noteTitleError(_title.text) : null,
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                key: const ValueKey<String>('note-text-field'),
                controller: _text,
                minLines: 6,
                maxLines: 12,
                decoration: InputDecoration(
                  labelText: l10n.noteMarkdown,
                  alignLabelWithHint: true,
                  border: OutlineInputBorder(),
                ),
              ),
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(l10n.cancel),
          ),
          FilledButton(onPressed: _submit, child: Text(l10n.save)),
        ],
      );
}

/// Lets the owner pick meetings that are in no project yet.
///
/// Outputs: The chosen meeting IDs (empty when cancelled). Side effects: Shows a dialog.
Future<List<String>> pickMeetings(
        BuildContext context, List<MeetingSummary> meetings) async =>
    await showDialog<List<String>>(
      context: context,
      builder: (BuildContext context) => _MeetingPicker(meetings: meetings),
    ) ??
    const <String>[];

/// A checklist of meetings.
final class _MeetingPicker extends StatefulWidget {
  const _MeetingPicker({required this.meetings});

  final List<MeetingSummary> meetings;

  @override
  State<_MeetingPicker> createState() => _MeetingPickerState();
}

/// Holds the ticked meetings.
final class _MeetingPickerState extends State<_MeetingPicker> {
  final Set<String> _chosen = <String>{};

  @override
  Widget build(BuildContext context) {
    final MaterialLocalizations localizations =
        MaterialLocalizations.of(context);
    return AlertDialog(
      title: Text(l10n.addMeetings),
      content: SizedBox(
        width: 480,
        child: widget.meetings.isEmpty
            ? Text(l10n.everyMeetingIsAlreadyInA)
            : ListView(
                shrinkWrap: true,
                children: <Widget>[
                  for (final MeetingSummary meeting in widget.meetings)
                    CheckboxListTile(
                      value: _chosen.contains(meeting.meetingId),
                      title: Text(meeting.title),
                      subtitle: Text(localizations
                          .formatMediumDate(meeting.createdAt.toLocal())),
                      onChanged: (bool? ticked) => setState(() {
                        if (ticked ?? false) {
                          _chosen.add(meeting.meetingId);
                        } else {
                          _chosen.remove(meeting.meetingId);
                        }
                      }),
                    ),
                ],
              ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(const <String>[]),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _chosen.isEmpty
              ? null
              : () => Navigator.of(context).pop(_chosen.toList()),
          child: Text(l10n.add),
        ),
      ],
    );
  }
}

/// The value of the "No project" choice, which no project ID can equal.
const String _noProject = '';

/// Opens the project chooser for a meeting from its app bar (Issue #85).
final class MoveToProjectButton extends StatelessWidget {
  /// Creates the action for `meetingId`.
  const MoveToProjectButton({
    required this.controller,
    required this.meetingId,
    super.key,
  });

  final ProjectsController controller;
  final String meetingId;

  @override
  Widget build(BuildContext context) => IconButton(
        key: const ValueKey<String>('move-to-project'),
        tooltip: l10n.moveToProject,
        icon: const Icon(Icons.drive_file_move_outline),
        onPressed: () => chooseProjectForMeeting(context,
            controller: controller, meetingId: meetingId),
      );
}

/// The chooser's value for "New project…".
const String _newProject = '\u0000new';

/// Asks which project a meeting belongs to, offering a new one, and moves it there (#94).
///
/// Inputs: A mounted context, the projects controller, and the meeting.
/// Outputs: Completes when the chooser (and any naming dialog) closes.
/// Side effects: Reads projects and memberships; may create a project; writes the meeting's
/// document; shows a snack bar with the outcome.
Future<void> chooseProjectForMeeting(
  BuildContext context, {
  required ProjectsController controller,
  required String meetingId,
}) =>
    chooseProjectForMeetings(context,
        controller: controller, meetingIds: <String>[meetingId]);

/// The chooser's current value when the meetings are in different projects.
const String _mixedProjects = '\u0000mixed';

/// Asks which project several meetings belong to and moves them all there (#216); the
/// one-meeting case is [chooseProjectForMeeting].
///
/// Inputs: A mounted context, the projects controller, and the meetings (at least one).
/// Outputs: Completes when the chooser (and any naming dialog) closes.
/// Side effects: Reads projects and memberships; may create a project; writes each meeting's
/// document; shows one snack bar with the outcome.
Future<void> chooseProjectForMeetings(
  BuildContext context, {
  required ProjectsController controller,
  required List<String> meetingIds,
}) async {
  final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
  await controller.loadProjects();
  final Map<String, String> memberships = await controller.meetingProjects();
  if (!context.mounted || meetingIds.isEmpty) {
    return;
  }
  final Set<String> places = <String>{
    for (final String meetingId in meetingIds)
      memberships[meetingId] ?? _noProject,
  };
  final String current = places.length == 1 ? places.single : _mixedProjects;
  String? chosen = await showDialog<String>(
    context: context,
    builder: (BuildContext context) => SimpleDialog(
      title: Text(l10n.project),
      children: <Widget>[
        RadioGroup<String>(
          groupValue: current,
          onChanged: (String? value) => Navigator.of(context).pop(value),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              RadioListTile<String>(
                value: _noProject,
                title: Text(l10n.noProject),
              ),
              // Subprojects sit indented under their parent (#142).
              for (final (:ProjectSummary project, :int depth)
                  in controller.projectTree)
                RadioListTile<String>(
                  value: project.projectId,
                  contentPadding: EdgeInsetsDirectional.only(
                      start: 16 + depth * _nestingIndent, end: 24),
                  title: Text(project.name),
                ),
            ],
          ),
        ),
        ListTile(
          key: const ValueKey<String>('choose-new-project'),
          leading: const Icon(Icons.create_new_folder_outlined),
          title: Text(l10n.newProject),
          onTap: () => Navigator.of(context).pop(_newProject),
        ),
      ],
    ),
  );
  if (chosen == _newProject) {
    if (!context.mounted) {
      return;
    }
    final String? name = await askProjectName(context,
        title: l10n.newProject2, confirmLabel: l10n.create);
    if (name == null) {
      return;
    }
    final ProjectSummary? created = await controller.createProject(name);
    if (created == null) {
      messenger.showSnackBar(
          SnackBar(content: Text(l10n.theProjectCouldNotBeCreated)));
      return;
    }
    chosen = created.projectId;
  }
  if (chosen == null || chosen == current) {
    return;
  }
  final String target = chosen;
  bool moved = true;
  for (final String meetingId in meetingIds) {
    if (memberships[meetingId] == target) {
      continue;
    }
    moved = await controller.moveMeeting(
            meetingId, target == _noProject ? null : target) &&
        moved;
  }
  final String? targetName = controller.projects
      .where((ProjectSummary project) => project.projectId == target)
      .map((ProjectSummary project) => project.name)
      .firstOrNull;
  final bool several = meetingIds.length > 1;
  messenger.showSnackBar(SnackBar(
    content: Text(!moved
        ? (several
            ? l10n.someMeetingsCouldNotBeMoved
            : l10n.theMeetingCouldNotBeMoved)
        : target == _noProject
            ? (several
                ? l10n.removedMeetingsFromTheirProjects(meetingIds.length)
                : l10n.removedFromItsProject)
            : several
                ? l10n.movedMeetingsTo(
                    meetingIds.length, targetName ?? 'the project')
                : l10n.movedTo(targetName ?? 'the project')),
  ));
}

/// How far each level of subprojects is indented in choosers (#142).
const double _nestingIndent = 24;

/// The parent chooser's value for the top level, which no project ID can equal.
const String _topLevel = '';

/// Asks where `project` should go (the top level or another project) and moves it there (#142).
///
/// Inputs: A mounted context, the projects controller and the project. The project itself and
/// its subprojects are not offered, nor its current place. Outputs: Completes when the chooser
/// closes. Side effects: May write the project's parent, which syncs; shows a snack bar with the
/// outcome.
Future<void> chooseProjectParent(
  BuildContext context, {
  required ProjectsController controller,
  required ProjectSummary project,
}) async {
  final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
  final String current = project.parentId ?? _topLevel;
  final String? chosen = await showDialog<String>(
    context: context,
    builder: (BuildContext context) => SimpleDialog(
      title: Text(l10n.moveProjectTitle(project.name)),
      children: <Widget>[
        ListTile(
          key: const ValueKey<String>('move-project-top-level'),
          leading: const Icon(Icons.vertical_align_top),
          title: Text(l10n.topLevel),
          enabled: current != _topLevel,
          onTap: () => Navigator.of(context).pop(_topLevel),
        ),
        for (final (project: ProjectSummary candidate, :int depth)
            in controller.projectTree)
          if (!controller.isWithin(candidate.projectId, project.projectId))
            ListTile(
              key: ValueKey<String>('move-project-into-${candidate.projectId}'),
              contentPadding: EdgeInsetsDirectional.only(
                  start: 16 + depth * _nestingIndent, end: 24),
              leading: const Icon(Icons.folder_outlined),
              title: Text(candidate.name),
              enabled: candidate.projectId != current,
              onTap: () => Navigator.of(context).pop(candidate.projectId),
            ),
      ],
    ),
  );
  if (chosen == null || chosen == current) {
    return;
  }
  final String? parentId = chosen == _topLevel ? null : chosen;
  final bool moved = await controller.moveProject(project.projectId, parentId);
  messenger.showSnackBar(SnackBar(
    content: Text(!moved
        ? l10n.theProjectCouldNotBeMoved
        : parentId == null
            ? l10n.movedToTopLevel
            : l10n.movedTo(controller.projectById(parentId)?.name ?? '')),
  ));
}
