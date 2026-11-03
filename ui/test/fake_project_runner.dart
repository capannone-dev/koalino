// Provides a scripted `note_taker_project_command` for the Projects tests (Issue #85): it records
// each request and answers from a function of the command, so no Rust library is loaded.

import 'dart:convert';

/// Answers Projects commands without Rust.
final class FakeProjectRunner {
  /// Creates a runner whose answers come from `answer` (a JSON-ready object per request).
  FakeProjectRunner(this.answer);

  /// Builds the answer for one decoded request.
  Map<String, Object?> Function(Map<String, Object?> request) answer;

  /// Every request received, decoded, in order.
  final List<Map<String, Object?>> requests = <Map<String, Object?>>[];

  /// The `command` of every request, in order.
  List<String> get commands => <String>[
        for (final Map<String, Object?> request in requests)
          request['command']! as String,
      ];

  /// The runner to pass to `ProjectBridge`.
  Future<String> call(String requestJson) async {
    final Map<String, Object?> request =
        (jsonDecode(requestJson) as Map).cast<String, Object?>();
    requests.add(request);
    return jsonEncode(answer(request));
  }
}

/// A project as Rust sends it.
Map<String, Object?> projectJson(String projectId, String name,
        {int meetingCount = 0, int resourceCount = 0}) =>
    <String, Object?>{
      'project_id': projectId,
      'name': name,
      'created_at': '2026-11-03T09:00:00+00:00',
      'updated_at': '2026-11-03T09:00:00+00:00',
      'meeting_count': meetingCount,
      'resource_count': resourceCount,
      'latest_meeting_at': null,
    };

/// A resource as Rust sends it to the app.
Map<String, Object?> resourceJson(
  String attachmentId,
  String fileName,
  String kind, {
  String mimeType = 'application/octet-stream',
  String filePath = '/data/attachments/x',
  int? sizeBytes = 10,
}) =>
    <String, Object?>{
      'attachment_id': attachmentId,
      'file_name': fileName,
      'mime_type': mimeType,
      'kind': kind,
      'added_at': '2026-11-03T10:00:00+00:00',
      'size_bytes': sizeBytes,
      'file_path': filePath,
    };
