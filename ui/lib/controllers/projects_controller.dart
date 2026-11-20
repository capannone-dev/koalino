// Owns the Projects tab's state (Issue #85): the project list, the open project with its
// meetings and resources, and every change to them, all through the Rust project bridge.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/controllers/meeting_title_validation.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Checks a project name with the rules Rust applies (those of meeting titles).
///
/// Inputs: The name as typed. Outputs: A message to show under the field, or `null` when the
/// name is valid. Side effects: None.
String? projectNameError(String rawName) =>
    switch (validateMeetingTitle(rawName).error) {
      null => null,
      MeetingTitleError.empty => l10n.enterAName,
      MeetingTitleError.tooLong =>
        l10n.namesCanBeAtMostCharacters(maxMeetingTitleCharacters),
      MeetingTitleError.controlCharacter => l10n.namesCanTContainLineBreaks,
    };

/// Checks a project note's title with the same rules, worded for a title (#124).
///
/// Inputs: The title as typed. Outputs: A message to show under the field, or `null` when the
/// title is valid. Side effects: None.
String? noteTitleError(String rawTitle) =>
    switch (validateMeetingTitle(rawTitle).error) {
      null => null,
      MeetingTitleError.empty => l10n.enterATitle,
      MeetingTitleError.tooLong =>
        l10n.titlesCanBeAtMost(maxMeetingTitleCharacters),
      MeetingTitleError.controlCharacter => l10n.titlesCanTContainLineBreaks,
    };

/// Lists projects, opens one, and changes them.
final class ProjectsController extends ChangeNotifier {
  /// Creates the controller over the project bridge, the meeting bridge (to offer meetings to
  /// add) and the file picker.
  ProjectsController({
    required ProjectBridge projectBridge,
    required MeetingBridge meetingBridge,
    required ProjectFilesPicker filesPicker,
  })  : _projectBridge = projectBridge,
        _meetingBridge = meetingBridge,
        _filesPicker = filesPicker;

  final ProjectBridge _projectBridge;
  final MeetingBridge _meetingBridge;
  final ProjectFilesPicker _filesPicker;
  List<ProjectSummary> _projects = const <ProjectSummary>[];
  bool _isLoading = false;
  String? _errorMessage;
  String? _openProjectId;
  ProjectDetail? _openProject;
  bool _isLoadingDetail = false;
  String? _detailErrorMessage;
  bool _isBusy = false;

  /// Projects by name.
  List<ProjectSummary> get projects => _projects;

  /// Whether the list is loading.
  bool get isLoading => _isLoading;

  /// A message for the latest failed list load, or `null`.
  String? get errorMessage => _errorMessage;

  /// The project being shown, or `null`.
  String? get openProjectId => _openProjectId;

  /// The loaded project, or `null` while loading or after a failure.
  ProjectDetail? get openDetail => _openProject;

  /// Whether the open project is loading.
  bool get isLoadingDetail => _isLoadingDetail;

  /// A message for the latest failed project load, or `null`.
  String? get detailErrorMessage => _detailErrorMessage;

  /// Whether files or a note are being added; the actions are disabled meanwhile.
  bool get isBusy => _isBusy;

  /// Loads (or reloads) the project list.
  ///
  /// Side effects: Reads SQLite through the bridge; notifies listeners. Rust's error details are
  /// not shown.
  Future<void> loadProjects() async {
    _isLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      _projects = await _projectBridge.listProjects();
    } on Object {
      _errorMessage = l10n.unableToLoadProjects;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  /// Opens `projectId` and loads its meetings and resources.
  ///
  /// Outputs: Completes once the project or a message is available; an answer for an earlier
  /// project is dropped. Side effects: Reads SQLite; notifies listeners.
  Future<void> openProject(String projectId) async {
    _openProjectId = projectId;
    _openProject = null;
    _detailErrorMessage = null;
    _isLoadingDetail = true;
    notifyListeners();
    await _reloadOpenProject();
  }

  /// Reloads the open project, if any, without clearing what is shown.
  Future<void> _reloadOpenProject() async {
    final String? projectId = _openProjectId;
    if (projectId == null) {
      return;
    }
    ProjectDetail? loaded;
    String? loadError;
    try {
      loaded = await _projectBridge.projectDetail(projectId);
    } on Object {
      loadError = l10n.unableToLoadThisProject;
    }
    if (_openProjectId != projectId) {
      return;
    }
    _openProject = loaded;
    _detailErrorMessage = loadError;
    _isLoadingDetail = false;
    notifyListeners();
  }

  /// Refreshes the list and the open project.
  Future<void> refresh() async {
    await loadProjects();
    await _reloadOpenProject();
  }

  /// Creates a project.
  ///
  /// Inputs: A name already checked with `projectNameError`.
  /// Outputs: The new project, or `null` when Rust refused. Side effects: Writes SQLite; reloads
  /// the list.
  Future<ProjectSummary?> createProject(String name, {String? parentId}) async {
    try {
      final ProjectSummary created =
          await _projectBridge.createProject(name.trim(), parentId: parentId);
      await loadProjects();
      return created;
    } on Object {
      return null;
    }
  }

  /// Moves a project inside `parentId`, or to the top level when null (#142).
  ///
  /// Outputs: Whether it moved. Side effects: Writes SQLite; reloads the list and the open
  /// project.
  Future<bool> moveProject(String projectId, String? parentId) async {
    try {
      await _projectBridge.moveProject(projectId, parentId);
    } on Object {
      return false;
    }
    await refresh();
    return true;
  }

  /// The project with `projectId`, or `null` when it is not listed.
  ProjectSummary? projectById(String projectId) => _projects
      .where((ProjectSummary project) => project.projectId == projectId)
      .firstOrNull;

  /// The projects directly inside `parentId` (the top level when null), by name.
  List<ProjectSummary> childrenOf(String? parentId) => <ProjectSummary>[
        for (final ProjectSummary project in _projects)
          if (_shownParent(project) == parentId) project,
      ];

  /// Every project in tree order (each followed by its subprojects) with its depth from the top
  /// level, for choosers.
  List<({ProjectSummary project, int depth})> get projectTree {
    final List<({ProjectSummary project, int depth})> ordered =
        <({ProjectSummary project, int depth})>[];
    final Set<String> seen = <String>{};
    void visit(String? parentId, int depth) {
      for (final ProjectSummary child in childrenOf(parentId)) {
        if (seen.add(child.projectId)) {
          ordered.add((project: child, depth: depth));
          visit(child.projectId, depth + 1);
        }
      }
    }

    visit(null, 0);
    return ordered;
  }

  /// Whether `projectId` is `ancestorId` or somewhere inside it.
  bool isWithin(String projectId, String ancestorId) {
    String? current = projectId;
    for (int step = 0; step <= _projects.length && current != null; step++) {
      if (current == ancestorId) {
        return true;
      }
      current = projectById(current)?.parentId;
    }
    return false;
  }

  /// The parent a project shows under: its parent when listed, the top level otherwise.
  String? _shownParent(ProjectSummary project) => switch (project.parentId) {
        final String parentId when projectById(parentId) != null => parentId,
        _ => null,
      };

  /// Renames a project.
  ///
  /// Outputs: Whether it was renamed. Side effects: Writes SQLite; reloads the list and the open
  /// project.
  Future<bool> renameProject(String projectId, String name) async {
    try {
      await _projectBridge.renameProject(projectId, name.trim());
    } on Object {
      return false;
    }
    await refresh();
    return true;
  }

  /// Deletes a project; its meetings stay, outside any project.
  ///
  /// Outputs: Whether it was deleted. Side effects: Deletes rows and files; closes it when open;
  /// reloads the list.
  Future<bool> deleteProject(String projectId) async {
    try {
      await _projectBridge.deleteProject(projectId);
    } on Object {
      return false;
    }
    if (_openProjectId == projectId) {
      _openProjectId = null;
      _openProject = null;
    }
    await loadProjects();
    return true;
  }

  /// Lets the owner pick files and adds them to `projectId`.
  ///
  /// Outputs: A message for a snack bar, or `null` when the picker was cancelled.
  /// Side effects: Opens the picker; Rust copies the files; reloads the project.
  Future<String?> addFiles(String projectId) async {
    if (_isBusy) {
      return null;
    }
    _setBusy(true);
    try {
      final List<String> filePaths = await _filesPicker.pickFiles();
      if (filePaths.isEmpty) {
        return null;
      }
      return await addPaths(projectId, filePaths);
    } on Object {
      return l10n.theFilesCouldNotBeAdded;
    } finally {
      _setBusy(false);
    }
  }

  /// Adds files already on the device (picked, or shared from another app) to `projectId`.
  ///
  /// Outputs: A message for a snack bar. Side effects: Rust copies the files; reloads the project
  /// and the list.
  Future<String> addPaths(String projectId, List<String> filePaths) async {
    try {
      final AddProjectFilesResult result =
          await _projectBridge.addFiles(projectId, filePaths);
      await refresh();
      final int addedCount = result.added.length;
      final List<String> parts = <String>[
        l10n.addedFiles(addedCount),
        if (result.failedPaths.isNotEmpty)
          l10n.couldNotBeAdded(result.failedPaths.length),
        if (result.skippedAttachments > 0)
          l10n.emailAttachmentsLeft(result.skippedAttachments),
      ];
      return parts.join('; ');
    } on Object {
      return l10n.theFilesCouldNotBeAdded;
    }
  }

  /// Writes a typed note into `projectId` as a Markdown file.
  ///
  /// Outputs: Whether it was saved. Side effects: Writes a file and SQLite; reloads the project.
  Future<bool> addNote(String projectId, String title, String text) async {
    _setBusy(true);
    try {
      await _projectBridge.addNote(projectId, title.trim(), text);
      await refresh();
      return true;
    } on Object {
      return false;
    } finally {
      _setBusy(false);
    }
  }

  /// Deletes one resource on every paired device.
  ///
  /// Outputs: Whether it was deleted. Side effects: Deletes the row and file; reloads.
  Future<bool> deleteResource(String projectId, String attachmentId) async {
    try {
      await _projectBridge.deleteResource(projectId, attachmentId);
    } on Object {
      return false;
    }
    await refresh();
    return true;
  }

  /// Reads an e-mail resource as text; throws when it cannot be read.
  Future<EmailView> readEmail(String projectId, String attachmentId) =>
      _projectBridge.readEmail(projectId, attachmentId);

  /// Moves a meeting into `projectId`, or out of any project with `null`.
  ///
  /// Outputs: Whether it moved. Side effects: Writes the meeting's document; reloads.
  Future<bool> moveMeeting(String meetingId, String? projectId) async {
    try {
      await _projectBridge.moveMeeting(meetingId, projectId);
    } on Object {
      return false;
    }
    await refresh();
    return true;
  }

  /// The project each meeting is in; empty when it cannot be read.
  Future<Map<String, String>> meetingProjects() async {
    try {
      return await _projectBridge.meetingProjects();
    } on Object {
      return const <String, String>{};
    }
  }

  /// Meetings in no project yet, newest first, to offer when adding meetings to a project.
  ///
  /// Outputs: The meetings; empty when they cannot be read. Side effects: Reads SQLite.
  Future<List<MeetingSummary>> meetingsOutsideProjects() async {
    try {
      final List<MeetingSummary> meetings = await _meetingBridge.getMeetings();
      final Map<String, String> memberships =
          await _projectBridge.meetingProjects();
      return meetings
          .where((MeetingSummary meeting) =>
              !memberships.containsKey(meeting.meetingId))
          .toList(growable: false)
          .reversed
          .toList(growable: false);
    } on Object {
      return const <MeetingSummary>[];
    }
  }

  /// Marks an add in progress or done; notifies listeners.
  void _setBusy(bool isBusy) {
    _isBusy = isBusy;
    notifyListeners();
  }
}
