// The search page (#100), opened from the Meetings screen (every meeting), the Projects screen
// (every project) and a project's page (that project). Each result names its meeting or project
// and where the match is, marks the matched words, and opens the meeting at that passage or the
// file through the caller's `onOpen`.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/infrastructure/search_bridge.dart';
import 'package:note_taker_ui/presentation/resource_viewer_screen.dart';

/// Opens a result: the meeting at its passage, or the file.
typedef OpenSearchHit = void Function(BuildContext context, SearchHit hit);

/// Builds a meeting's page scrolled to `startTimeMs` (null for the top).
typedef MeetingMomentBuilder = Widget Function(
    BuildContext context, String meetingId, int? startTimeMs);

/// Searches and lists the results.
final class SearchScreen extends StatefulWidget {
  /// Creates the page.
  const SearchScreen({
    required this.bridge,
    required this.scope,
    required this.title,
    required this.onOpen,
    this.projectId,
    super.key,
  });

  final SearchBridge bridge;
  final SearchScope scope;

  /// The project to search, for `SearchScope.project`.
  final String? projectId;

  /// The page title and the field's label, such as "Search meetings".
  final String title;
  final OpenSearchHit onOpen;

  @override
  State<SearchScreen> createState() => _SearchScreenState();
}

/// Keeps the query, the results and the status.
final class _SearchScreenState extends State<SearchScreen> {
  final TextEditingController _query = TextEditingController();
  SearchResults? _results;
  String? _status;
  bool _searching = false;

  @override
  void dispose() {
    _query.dispose();
    super.dispose();
  }

  /// Runs the search. Side effects: One core call; nothing is stored.
  Future<void> _search() async {
    setState(() {
      _searching = true;
      _status = 'Searching…';
    });
    SearchResults? results;
    String status;
    try {
      results = await widget.bridge
          .search(_query.text, widget.scope, projectId: widget.projectId);
      final int count = results.hits.length;
      status = count == 0
          ? 'Nothing found.'
          : '${count == 1 ? '1 result' : '$count results'}'
              '${results.truncated ? ', more not shown: add words to narrow it' : ''}';
    } on SearchException catch (error) {
      status = error.message;
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _searching = false;
      _results = results;
      _status = status;
    });
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final List<SearchHit> hits = _results?.hits ?? const <SearchHit>[];
    return Scaffold(
      appBar: AppBar(title: Text(widget.title)),
      body: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 840),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
                child: TextField(
                  controller: _query,
                  autofocus: true,
                  textInputAction: TextInputAction.search,
                  onSubmitted: (_) => _search(),
                  decoration: InputDecoration(
                    labelText: widget.title,
                    border: const OutlineInputBorder(),
                    suffixIcon: IconButton(
                      tooltip: 'Search',
                      icon: const Icon(Icons.search),
                      onPressed: _searching ? null : _search,
                    ),
                  ),
                ),
              ),
              if (_status case final String status)
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: Semantics(
                    liveRegion: true,
                    child: Text(status,
                        style: theme.textTheme.bodyMedium?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant)),
                  ),
                ),
              if (_searching) const LinearProgressIndicator(),
              Expanded(
                child: ListView.separated(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  itemCount: hits.length,
                  separatorBuilder: (BuildContext context, int index) =>
                      const Divider(height: 1),
                  itemBuilder: (BuildContext context, int index) => _ResultTile(
                    hit: hits[index],
                    index: index,
                    onTap: () => widget.onOpen(context, hits[index]),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// One result: where it is, and the passage with the matched words marked.
final class _ResultTile extends StatelessWidget {
  const _ResultTile(
      {required this.hit, required this.index, required this.onTap});

  final SearchHit hit;
  final int index;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final TextStyle marked = TextStyle(
      backgroundColor: theme.colorScheme.tertiaryContainer,
      color: theme.colorScheme.onTertiaryContainer,
      fontWeight: FontWeight.w600,
    );
    return ListTile(
      onTap: onTap,
      leading: Icon(switch (hit.place) {
        SearchPlace.transcript ||
        SearchPlace.transcriptText =>
          Icons.record_voice_over_outlined,
        SearchPlace.summary => Icons.summarize_outlined,
        SearchPlace.notes => Icons.edit_note,
        SearchPlace.resourceName ||
        SearchPlace.resourceText =>
          Icons.description_outlined,
        SearchPlace.projectName => Icons.folder_outlined,
        SearchPlace.meetingTitle => Icons.event_note,
      }),
      title: Wrap(
        spacing: 8,
        crossAxisAlignment: WrapCrossAlignment.end,
        children: <Widget>[
          Text(hit.heading, style: theme.textTheme.titleSmall),
          Text(hit.placeLabel,
              style: theme.textTheme.bodySmall
                  ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
        ],
      ),
      subtitle: Text.rich(
        key: ValueKey<String>('search-snippet-$index'),
        TextSpan(children: <TextSpan>[
          for (final SnippetPart part in hit.snippet)
            TextSpan(text: part.text, style: part.matched ? marked : null),
        ]),
      ),
    );
  }
}

/// Opens a result found among projects: a meeting at its passage, a file in its viewer, or the
/// project's page.
///
/// Inputs: The page's context, the hit, the projects controller, a builder for meeting pages,
/// and one for the project's page (null when the search was opened from that page). Side effects:
/// Opens the project in the controller to find a file, and pushes a page.
Future<void> openProjectSearchHit(
  BuildContext context,
  SearchHit hit, {
  required ProjectsController controller,
  required MeetingMomentBuilder? meetingBuilder,
  required Widget Function(BuildContext context, String projectId)?
      projectPageBuilder,
}) async {
  final NavigatorState navigator = Navigator.of(context);
  if (hit.meetingId case final String meetingId) {
    if (meetingBuilder != null) {
      await navigator.push(MaterialPageRoute<void>(
        builder: (BuildContext context) =>
            meetingBuilder(context, meetingId, hit.startTimeMs),
      ));
    }
    return;
  }
  final String? projectId = hit.projectId;
  if (projectId == null) {
    return;
  }
  if (hit.attachmentId case final String attachmentId) {
    if (controller.openProjectId != projectId ||
        controller.openDetail == null) {
      await controller.openProject(projectId);
    }
    final ProjectResource? resource = controller.openDetail?.resources
        .where(
            (ProjectResource resource) => resource.attachmentId == attachmentId)
        .firstOrNull;
    if (resource != null && navigator.mounted) {
      await navigator.push(MaterialPageRoute<void>(
        builder: (BuildContext context) => ResourceViewerScreen(
          controller: controller,
          projectId: projectId,
          resource: resource,
        ),
      ));
      return;
    }
  }
  if (projectPageBuilder != null && navigator.mounted) {
    controller.openProject(projectId);
    await navigator.push(MaterialPageRoute<void>(
      builder: (BuildContext context) => projectPageBuilder(context, projectId),
    ));
  } else if (navigator.mounted) {
    navigator.pop();
  }
}
