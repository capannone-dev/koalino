// Dialogs of the Projects tab (Issue #85): naming a project, writing a note, choosing meetings
// to add, and choosing the project a meeting belongs to.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';

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
            labelText: 'Name',
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
            child: const Text('Cancel'),
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
    if (projectNameError(_title.text) != null) {
      setState(() => _showError = true);
      return;
    }
    Navigator.of(context).pop((_title.text.trim(), _text.text));
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
        title: const Text('New note'),
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
                  labelText: 'Title',
                  errorText: _showError
                      ? projectNameError(_title.text)
                          ?.replaceFirst('a name', 'a title')
                          .replaceFirst('Names', 'Titles')
                      : null,
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                key: const ValueKey<String>('note-text-field'),
                controller: _text,
                minLines: 6,
                maxLines: 12,
                decoration: const InputDecoration(
                  labelText: 'Note (Markdown)',
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
            child: const Text('Cancel'),
          ),
          FilledButton(onPressed: _submit, child: const Text('Save')),
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
      title: const Text('Add meetings'),
      content: SizedBox(
        width: 480,
        child: widget.meetings.isEmpty
            ? const Text('Every meeting is already in a project.')
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
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: _chosen.isEmpty
              ? null
              : () => Navigator.of(context).pop(_chosen.toList()),
          child: const Text('Add'),
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
        tooltip: 'Move to project',
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
}) async {
  final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
  await controller.loadProjects();
  final Map<String, String> memberships = await controller.meetingProjects();
  if (!context.mounted) {
    return;
  }
  final String current = memberships[meetingId] ?? _noProject;
  String? chosen = await showDialog<String>(
    context: context,
    builder: (BuildContext context) => SimpleDialog(
      title: const Text('Project'),
      children: <Widget>[
        RadioGroup<String>(
          groupValue: current,
          onChanged: (String? value) => Navigator.of(context).pop(value),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              const RadioListTile<String>(
                value: _noProject,
                title: Text('No project'),
              ),
              for (final ProjectSummary project in controller.projects)
                RadioListTile<String>(
                  value: project.projectId,
                  title: Text(project.name),
                ),
            ],
          ),
        ),
        ListTile(
          key: const ValueKey<String>('choose-new-project'),
          leading: const Icon(Icons.create_new_folder_outlined),
          title: const Text('New project…'),
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
        title: 'New project', confirmLabel: 'Create');
    if (name == null) {
      return;
    }
    final ProjectSummary? created = await controller.createProject(name);
    if (created == null) {
      messenger.showSnackBar(
          const SnackBar(content: Text('The project could not be created.')));
      return;
    }
    chosen = created.projectId;
  }
  if (chosen == null || chosen == current) {
    return;
  }
  final String target = chosen;
  final bool moved = await controller.moveMeeting(
      meetingId, target == _noProject ? null : target);
  final String? targetName = controller.projects
      .where((ProjectSummary project) => project.projectId == target)
      .map((ProjectSummary project) => project.name)
      .firstOrNull;
  messenger.showSnackBar(SnackBar(
    content: Text(!moved
        ? 'The meeting could not be moved.'
        : target == _noProject
            ? 'Removed from its project'
            : 'Moved to ${targetName ?? 'the project'}'),
  ));
}
