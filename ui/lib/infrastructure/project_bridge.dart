// Defines how Flutter reaches Projects in Rust (Issue #85): folders of meetings plus resources
// (pictures, Markdown, PDFs, e-mails and documents). Every call goes through
// `note_taker_project_command`, one JSON command answered with `{"ok":...}`, on a background
// isolate because commands read SQLite and copy files.

import 'dart:convert';
import 'dart:ffi';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:file_picker/file_picker.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';

import 'core_library.dart';

/// One project in the list.
final class ProjectSummary {
  /// Creates an immutable summary.
  const ProjectSummary({
    required this.projectId,
    required this.name,
    required this.meetingCount,
    required this.resourceCount,
    required this.latestMeetingAt,
    this.parentId,
  });

  /// Stable project identifier (a UUID).
  final String projectId;

  /// The name shown; 1–200 characters, one line.
  final String name;

  /// How many meetings it holds.
  final int meetingCount;

  /// How many resources it holds.
  final int resourceCount;

  /// When its newest meeting started, or `null` without meetings.
  final DateTime? latestMeetingAt;

  /// The project it shows inside, or `null` at the top level (#142).
  final String? parentId;

  /// Decodes one project object.
  ///
  /// Throws `TypeError` or `FormatException` for a malformed object.
  factory ProjectSummary.fromJson(Map<String, Object?> json) => ProjectSummary(
        projectId: json['project_id']! as String,
        name: json['name']! as String,
        meetingCount: (json['meeting_count'] as num?)?.toInt() ?? 0,
        resourceCount: (json['resource_count'] as num?)?.toInt() ?? 0,
        latestMeetingAt: switch (json['latest_meeting_at']) {
          final String time => DateTime.parse(time).toUtc(),
          _ => null,
        },
        parentId: json['parent_id'] as String?,
      );
}

/// How the app shows a resource.
enum ProjectResourceKind {
  /// A picture, shown full screen.
  image,

  /// A Markdown file, rendered in the app.
  markdown,

  /// A PDF, shown in the app.
  pdf,

  /// An e-mail, shown as headers and plain text.
  email,

  /// A plain text file.
  text,

  /// An office document the app cannot show.
  document;

  /// Reads the `kind` Rust sends; unknown kinds are documents.
  static ProjectResourceKind fromWire(Object? kind) =>
      ProjectResourceKind.values.firstWhere(
        (ProjectResourceKind candidate) => candidate.name == kind,
        orElse: () => ProjectResourceKind.document,
      );
}

/// One resource of a project.
final class ProjectResource {
  /// Creates an immutable resource.
  const ProjectResource({
    required this.attachmentId,
    required this.fileName,
    required this.mimeType,
    required this.kind,
    required this.addedAt,
    required this.sizeBytes,
    required this.filePath,
  });

  /// Stable resource identifier (a UUID).
  final String attachmentId;

  /// The name shown, as stored (reduced to safe characters).
  final String fileName;

  /// The stored MIME type.
  final String mimeType;

  /// How it is shown.
  final ProjectResourceKind kind;

  /// When it was added, on whichever device.
  final DateTime addedAt;

  /// Its size, or `null` while it is still on its way from a paired device.
  final int? sizeBytes;

  /// Its private local copy.
  final String filePath;

  /// Whether the file is here yet.
  bool get isAvailable => sizeBytes != null;

  /// Decodes one resource object.
  ///
  /// Throws `TypeError` or `FormatException` for a malformed object.
  factory ProjectResource.fromJson(Map<String, Object?> json) =>
      ProjectResource(
        attachmentId: json['attachment_id']! as String,
        fileName: json['file_name']! as String,
        mimeType: json['mime_type']! as String,
        kind: ProjectResourceKind.fromWire(json['kind']),
        addedAt: DateTime.parse(json['added_at']! as String).toUtc(),
        sizeBytes: (json['size_bytes'] as num?)?.toInt(),
        filePath: (json['file_path'] as String?) ?? '',
      );
}

/// A project with what it holds.
final class ProjectDetail {
  /// Creates an immutable detail.
  const ProjectDetail({
    required this.project,
    required this.meetings,
    required this.resources,
  });

  /// The project and its counts.
  final ProjectSummary project;

  /// Its meetings, newest first.
  final List<MeetingSummary> meetings;

  /// Its resources, oldest first.
  final List<ProjectResource> resources;
}

/// One person in an e-mail header.
final class EmailPerson {
  /// Creates an immutable person.
  const EmailPerson({required this.name, required this.address});

  /// The display name, when given.
  final String? name;

  /// The address, when given.
  final String? address;

  /// `Name <address>`, or whichever part exists.
  String get label => switch ((name, address)) {
        (final String name, final String address) => '$name <$address>',
        (final String name, null) => name,
        (null, final String address) => address,
        (null, null) => '',
      };
}

/// An e-mail resource as text.
final class EmailView {
  /// Creates an immutable e-mail.
  const EmailView({
    required this.subject,
    required this.from,
    required this.to,
    required this.cc,
    required this.sentAt,
    required this.bodyText,
    required this.bodyTruncated,
    required this.attachmentNames,
  });

  /// The subject, when present.
  final String? subject;

  /// Senders, direct recipients and copied recipients.
  final List<EmailPerson> from;
  final List<EmailPerson> to;
  final List<EmailPerson> cc;

  /// When it was sent, when readable.
  final DateTime? sentAt;

  /// The body as plain text (HTML-only mail was turned into text by Rust).
  final String bodyText;

  /// Whether the body was cut short.
  final bool bodyTruncated;

  /// The names of its attachments.
  final List<String> attachmentNames;
}

/// What adding files achieved.
final class AddProjectFilesResult {
  /// Creates an immutable result.
  const AddProjectFilesResult({
    required this.added,
    required this.failedPaths,
    required this.skippedAttachments,
  });

  /// Resources stored, e-mail attachments included.
  final List<ProjectResource> added;

  /// Paths that could not be added.
  final List<String> failedPaths;

  /// E-mail attachments of a type a project does not accept.
  final int skippedAttachments;
}

/// Why a Projects command failed.
final class ProjectCommandException implements Exception {
  /// Creates the exception from Rust's `error_code`.
  const ProjectCommandException(this.errorCode);

  /// `invalid_name`, `project_not_found`, `resource_not_found`, `rejected`, and so on.
  final String errorCode;

  @override
  String toString() => 'ProjectCommandException($errorCode)';
}

/// Decodes an answer: the object for `{"ok":true,...}`; throws `ProjectCommandException` for
/// `{"ok":false,...}` and `FormatException` for any other shape.
Map<String, Object?> decodeProjectAnswer(String answerJson) {
  final Object? decoded = jsonDecode(answerJson);
  if (decoded is! Map) {
    throw const FormatException('A project answer must be a JSON object.');
  }
  final Map<String, Object?> answer = decoded.cast<String, Object?>();
  if (answer['ok'] != true) {
    throw ProjectCommandException(
        (answer['error_code'] as String?) ?? 'unknown_error');
  }
  return answer;
}

/// Decodes a list of JSON objects with `decode`.
List<T> _decodeList<T>(
        Object? json, T Function(Map<String, Object?> object) decode) =>
    ((json as List<Object?>?) ?? const <Object?>[])
        .map((Object? item) => decode((item! as Map).cast<String, Object?>()))
        .toList(growable: false);

/// Decodes the `detail` answer.
ProjectDetail decodeProjectDetail(Map<String, Object?> answer) => ProjectDetail(
      project: ProjectSummary.fromJson(
          (answer['project']! as Map).cast<String, Object?>()),
      meetings: _decodeList(answer['meetings'], MeetingSummary.fromJson),
      resources: _decodeList(answer['resources'], ProjectResource.fromJson),
    );

/// Decodes the `read_email` answer's `email` object.
EmailView decodeEmailView(Map<String, Object?> email) {
  List<EmailPerson> people(Object? json) => _decodeList(
        json,
        (Map<String, Object?> person) => EmailPerson(
          name: person['name'] as String?,
          address: person['address'] as String?,
        ),
      );
  return EmailView(
    subject: email['subject'] as String?,
    from: people(email['from']),
    to: people(email['to']),
    cc: people(email['cc']),
    sentAt: switch (email['sent_at']) {
      final String time => DateTime.parse(time).toUtc(),
      _ => null,
    },
    bodyText: (email['body_text'] as String?) ?? '',
    bodyTruncated: email['body_truncated'] == true,
    attachmentNames: _decodeList(
        email['attachments'],
        (Map<String, Object?> attachment) =>
            attachment['file_name']! as String),
  );
}

/// Runs one Projects command and returns its raw JSON answer.
typedef ProjectCommandRunner = Future<String> Function(String requestJson);

/// Reaches Projects in Rust.
final class ProjectBridge {
  /// Creates the bridge; tests pass a fake `runCommand`.
  const ProjectBridge({ProjectCommandRunner runCommand = runRustProjectCommand})
      : _runCommand = runCommand;

  final ProjectCommandRunner _runCommand;

  /// Sends `request` and decodes the answer; throws `ProjectCommandException` on refusal.
  Future<Map<String, Object?>> _send(Map<String, Object?> request) async =>
      decodeProjectAnswer(await _runCommand(jsonEncode(request)));

  /// Lists projects by name. Side effects: Reads SQLite.
  Future<List<ProjectSummary>> listProjects() async => _decodeList(
      (await _send(<String, Object?>{'command': 'list'}))['projects'],
      ProjectSummary.fromJson);

  /// Creates a project named `name`. Side effects: Writes SQLite; syncs to paired devices.
  /// Inside `parentId` when given (#142).
  Future<ProjectSummary> createProject(String name, {String? parentId}) async =>
      ProjectSummary.fromJson(((await _send(<String, Object?>{
        'command': 'create',
        'name': name,
        if (parentId != null) 'parent_id': parentId,
      }))['project']! as Map)
          .cast<String, Object?>());

  /// Moves a project inside `parentId`, or to the top level when null (#142).
  ///
  /// Throws `ProjectCommandException` with `invalid_parent` for a move inside itself.
  /// Side effects: Writes SQLite; syncs to paired devices.
  Future<void> moveProject(String projectId, String? parentId) =>
      _send(<String, Object?>{
        'command': 'move_project',
        'project_id': projectId,
        'parent_id': parentId,
      });

  /// Renames a project. Side effects: Writes SQLite; syncs to paired devices.
  Future<void> renameProject(String projectId, String name) =>
      _send(<String, Object?>{
        'command': 'rename',
        'project_id': projectId,
        'name': name
      });

  /// Deletes a project; its meetings stay, outside any project, and its resources go.
  /// Side effects: Writes SQLite, removes files; paired devices delete it too.
  Future<void> deleteProject(String projectId) =>
      _send(<String, Object?>{'command': 'delete', 'project_id': projectId});

  /// Loads a project's meetings and resources. Side effects: Reads SQLite.
  Future<ProjectDetail> projectDetail(String projectId) async =>
      decodeProjectDetail(await _send(
          <String, Object?>{'command': 'detail', 'project_id': projectId}));

  /// Moves a meeting into `projectId`, or out of any project with `null`.
  /// Side effects: Writes the meeting's document; syncs to paired devices.
  Future<void> moveMeeting(String meetingId, String? projectId) =>
      _send(<String, Object?>{
        'command': 'move_meeting',
        'meeting_id': meetingId,
        'project_id': projectId,
      });

  /// Maps each meeting in a project to its project ID. Side effects: Reads SQLite.
  Future<Map<String, String>> meetingProjects() async =>
      (((await _send(<String, Object?>{'command': 'meeting_projects'}))[
                  'meeting_projects'] as Map?) ??
              const <String, String>{})
          .cast<String, String>();

  /// Copies files into a project; an `.eml` also adds its attachments.
  /// Side effects: Copies files and writes SQLite; syncs to paired devices.
  Future<AddProjectFilesResult> addFiles(
      String projectId, List<String> filePaths) async {
    final Map<String, Object?> answer = await _send(<String, Object?>{
      'command': 'add_files',
      'project_id': projectId,
      'paths': filePaths,
    });
    return AddProjectFilesResult(
      added: _decodeList(answer['added'], ProjectResource.fromJson),
      failedPaths: _decodeList(answer['failed'],
          (Map<String, Object?> failure) => failure['path']! as String),
      skippedAttachments: (answer['skipped_attachments'] as num?)?.toInt() ?? 0,
    );
  }

  /// Writes a typed note into a project as Markdown.
  /// Side effects: Writes a file and SQLite; syncs to paired devices.
  Future<ProjectResource> addNote(
          String projectId, String title, String text) async =>
      ProjectResource.fromJson(((await _send(<String, Object?>{
        'command': 'add_note',
        'project_id': projectId,
        'title': title,
        'text': text,
      }))['resource']! as Map)
          .cast<String, Object?>());

  /// Deletes one resource and its file; paired devices delete it too.
  Future<void> deleteResource(String projectId, String attachmentId) =>
      _send(<String, Object?>{
        'command': 'delete_resource',
        'project_id': projectId,
        'attachment_id': attachmentId,
      });

  /// Reads an e-mail resource as plain text. Side effects: Reads the file.
  Future<EmailView> readEmail(String projectId, String attachmentId) async =>
      decodeEmailView(((await _send(<String, Object?>{
        'command': 'read_email',
        'project_id': projectId,
        'attachment_id': attachmentId,
      }))['email']! as Map)
          .cast<String, Object?>());
}

/// Runs `note_taker_project_command` on a background isolate and copies the answer out,
/// freeing every native buffer.
Future<String> runRustProjectCommand(String requestJson) => Isolate.run(() {
      final DynamicLibrary coreLibrary = _openCoreLibrary();
      final _DartProjectCommand projectCommand = coreLibrary.lookupFunction<
          _NativeProjectCommand,
          _DartProjectCommand>('note_taker_project_command');
      final _DartFreeString freeString =
          coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
              'note_taker_free_string');
      final Pointer<Utf8> nativeRequest = requestJson.toNativeUtf8();
      try {
        final Pointer<Utf8> answerPointer = projectCommand(nativeRequest);
        if (answerPointer == nullptr) {
          throw StateError('Rust returned no project answer.');
        }
        try {
          return answerPointer.toDartString();
        } finally {
          freeString(answerPointer);
        }
      } finally {
        calloc.free(nativeRequest);
      }
    });

/// Opens the platform-specific core library.
DynamicLibrary _openCoreLibrary() {
  return openCoreLibrary();
}

/// Picks files to add to a project.
abstract interface class ProjectFilesPicker {
  /// Opens the platform picker for the accepted types.
  ///
  /// Outputs: The selected absolute paths; empty when the owner cancels.
  /// Side effects: Opens a platform-owned picker.
  Future<List<String>> pickFiles();
}

/// Uses the standard cross-platform picker with multiple selection.
final class PlatformProjectFilesPicker implements ProjectFilesPicker {
  /// Creates the picker.
  const PlatformProjectFilesPicker();

  @override
  Future<List<String>> pickFiles() async {
    final FilePickerResult? selection = await FilePicker.platform.pickFiles(
      // A project takes any file (#94); types the app cannot show open in another app.
      allowMultiple: true,
    );
    return <String>[
      for (final PlatformFile file
          in selection?.files ?? const <PlatformFile>[])
        if (file.path case final String path) path,
    ];
  }
}

/// Represents `char *note_taker_project_command(const char *request_json)`.
typedef _NativeProjectCommand = Pointer<Utf8> Function(Pointer<Utf8> request);

/// Represents the Dart callable form of `note_taker_project_command`.
typedef _DartProjectCommand = Pointer<Utf8> Function(Pointer<Utf8> request);

/// Represents `void note_taker_free_string(char *value)`.
typedef _NativeFreeString = Void Function(Pointer<Utf8> value);

/// Represents the Dart callable form of `note_taker_free_string`.
typedef _DartFreeString = void Function(Pointer<Utf8> value);
