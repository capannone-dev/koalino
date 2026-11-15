// Calls the core's search (#100): where words were said or written in meetings and projects.
// One JSON request goes through `note_taker_search` on a background isolate, since search reads
// and unseals every meeting it looks at. The query is never logged.

import 'dart:convert';

import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// What to search.
enum SearchScope {
  /// Every meeting's title, summary, notes and transcript.
  meetings,

  /// Every project's name, files and meetings.
  projects,

  /// One project's name, files and meetings.
  project,
}

/// Where a match is.
enum SearchPlace {
  meetingTitle,
  summary,
  notes,
  transcript,
  transcriptText,
  projectName,
  resourceName,
  resourceText;

  /// Reads the core's name, such as `meeting_title`.
  static SearchPlace fromJson(String name) => switch (name) {
        'meeting_title' => meetingTitle,
        'summary' => summary,
        'notes' => notes,
        'transcript' => transcript,
        'transcript_text' => transcriptText,
        'project_name' => projectName,
        'resource_name' => resourceName,
        _ => resourceText,
      };
}

/// A piece of a snippet; `matched` marks a matched word.
final class SnippetPart {
  /// Creates the part.
  const SnippetPart(this.text, {required this.matched});

  final String text;
  final bool matched;
}

/// One match.
final class SearchHit {
  /// Creates the hit.
  const SearchHit({
    required this.place,
    required this.snippet,
    this.meetingId,
    this.meetingTitle,
    this.projectId,
    this.projectName,
    this.attachmentId,
    this.fileName,
    this.startTimeMs,
    this.speaker,
  });

  /// Reads a hit from the core's JSON.
  factory SearchHit.fromJson(Map<String, Object?> json) {
    final Map<String, Object?>? meeting =
        json['meeting'] as Map<String, Object?>?;
    final Map<String, Object?>? project =
        json['project'] as Map<String, Object?>?;
    final Map<String, Object?>? resource =
        json['resource'] as Map<String, Object?>?;
    return SearchHit(
      place: SearchPlace.fromJson(json['place'] as String? ?? ''),
      snippet: <SnippetPart>[
        for (final Object? part
            in json['snippet'] as List<Object?>? ?? <Object?>[])
          if (part case final Map<String, Object?> part)
            SnippetPart(part['text'] as String? ?? '',
                matched: part['match'] == true),
      ],
      meetingId: meeting?['meeting_id'] as String?,
      meetingTitle: meeting?['title'] as String?,
      projectId: project?['project_id'] as String?,
      projectName: project?['name'] as String?,
      attachmentId: resource?['attachment_id'] as String?,
      fileName: resource?['file_name'] as String?,
      startTimeMs: (json['start_time_ms'] as num?)?.toInt(),
      speaker: json['speaker'] as String?,
    );
  }

  final SearchPlace place;
  final List<SnippetPart> snippet;

  /// The meeting, unless the hit is in a project's name or file.
  final String? meetingId;
  final String? meetingTitle;

  /// The project, when searching projects.
  final String? projectId;
  final String? projectName;

  /// The file, for a hit in a project file.
  final String? attachmentId;
  final String? fileName;

  /// Where in the meeting a transcript hit starts.
  final int? startTimeMs;
  final String? speaker;

  /// The meeting's title, or the project's name.
  String get heading => meetingTitle ?? projectName ?? '';

  /// Where in its meeting or project the hit is, such as `1:01 · Anna` or `In plan.md`.
  String get placeLabel => switch (place) {
        SearchPlace.meetingTitle => l10n.title,
        SearchPlace.summary => l10n.summary,
        SearchPlace.notes => l10n.notes,
        SearchPlace.transcript =>
          '${formatMeetingDuration(startTimeMs ?? 0)} · ${speaker ?? ''}',
        SearchPlace.transcriptText => l10n.transcript,
        SearchPlace.projectName => l10n.projectName,
        SearchPlace.resourceName => l10n.file(fileName ?? ''),
        SearchPlace.resourceText => 'In ${fileName ?? ''}',
      };
}

/// The hits of one search.
final class SearchResults {
  /// Creates the results.
  const SearchResults({required this.hits, required this.truncated});

  final List<SearchHit> hits;

  /// Whether more hits were left out.
  final bool truncated;
}

/// A refused or failed search, with the core's stable error code.
final class SearchException implements Exception {
  /// Creates the exception.
  const SearchException(this.code);

  final String code;

  /// A sentence for people; the core's own text is not shown.
  String get message => switch (code) {
        'empty' => l10n.typeWhatToLookFor,
        'too_short' => l10n.typeAtLeastTwoLetters,
        'too_long' => l10n.theSearchIsTooLong,
        'project_not_found' => l10n.thatProjectNoLongerExists,
        'storage_failed' => l10n.theNotesCouldNotBeRead,
        'runtime_unavailable' => l10n.theAppIsStillStartingTry,
        _ => l10n.somethingWentWrongTryAgain,
      };

  @override
  String toString() => 'SearchException($code)';
}

/// Searches through the core.
final class SearchBridge {
  /// Creates the bridge; `call` is replaced in tests.
  const SearchBridge({LlmNativeCall call = callRustLlm}) : _call = call;

  final LlmNativeCall _call;

  /// Searches `scope` (one project needs `projectId`).
  ///
  /// Outputs: The hits. Throws `SearchException`. Side effects: The core reads (and unseals)
  /// meetings and project files; nothing is kept.
  Future<SearchResults> search(String query, SearchScope scope,
      {String? projectId}) async {
    final Map<String, Object?> request = <String, Object?>{
      'query': query,
      'scope': scope.name,
      if (projectId != null) 'project_id': projectId,
    };
    final Map<String, Object?> answer;
    try {
      answer = decodeLlmAnswer(
          await _call('note_taker_search', <String?>[jsonEncode(request)]));
    } on LlmException catch (error) {
      throw SearchException(error.code);
    }
    return SearchResults(
      hits: <SearchHit>[
        for (final Object? hit
            in answer['hits'] as List<Object?>? ?? <Object?>[])
          if (hit case final Map<String, Object?> hit) SearchHit.fromJson(hit),
      ],
      truncated: answer['truncated'] == true,
    );
  }
}
