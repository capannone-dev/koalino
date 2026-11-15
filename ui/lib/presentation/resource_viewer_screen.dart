// Shows one project resource inside the app (Issue #85): pictures full screen, Markdown rendered,
// PDFs with PDFium (`pdfrx`), e-mails as headers and plain text, and text files as text. Office
// documents cannot be shown and say so. Files are sealed at rest, so each view reads a short-lived
// clear copy the core makes (#99). Nothing here reaches the network.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/presentation/clear_file.dart';
import 'package:note_taker_ui/presentation/markdown_view.dart';
import 'package:pdfrx/pdfrx.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Builds the PDF view; tests replace it, since PDFium is not loaded there.
typedef PdfViewBuilder = Widget Function(String filePath);

/// The default PDF view: every page, zoomable.
Widget defaultPdfView(String filePath) => PdfViewer.file(filePath);

/// A full-screen viewer for `resource`.
final class ResourceViewerScreen extends StatelessWidget {
  /// Creates the viewer.
  ///
  /// Inputs: The controller (to read e-mails), the project, the resource, and optionally another
  /// PDF view. Outputs: A Material 3 screen. Side effects: Reads the file when shown.
  const ResourceViewerScreen({
    required this.controller,
    required this.projectId,
    required this.resource,
    this.pdfViewBuilder = defaultPdfView,
    super.key,
  });

  final ProjectsController controller;
  final String projectId;
  final ProjectResource resource;
  final PdfViewBuilder pdfViewBuilder;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(resource.fileName)),
      body: !resource.isAvailable
          ? _Message(
              icon: Icons.sync,
              text: l10n.thisFileIsStillOnIts,
            )
          : switch (resource.kind) {
              ProjectResourceKind.image => _Clear(
                  resource: resource,
                  builder: (String path) => InteractiveViewer(
                    maxScale: 8,
                    child: Center(
                      child: Image.file(
                        File(path),
                        semanticLabel: resource.fileName,
                        errorBuilder: (_, __, ___) => _Message(
                          icon: Icons.broken_image_outlined,
                          text: l10n.thisPictureCannotBeShown,
                        ),
                      ),
                    ),
                  ),
                ),
              ProjectResourceKind.markdown => _Clear(
                  resource: resource,
                  builder: (String path) => _FileText(
                    filePath: path,
                    builder: (String text) => MarkdownView(markdownText: text),
                  ),
                ),
              ProjectResourceKind.text => _Clear(
                  resource: resource,
                  builder: (String path) => _FileText(
                    filePath: path,
                    builder: (String text) => SingleChildScrollView(
                      padding: const EdgeInsets.all(16),
                      child: SelectableText(text),
                    ),
                  ),
                ),
              ProjectResourceKind.pdf =>
                _Clear(resource: resource, builder: pdfViewBuilder),
              ProjectResourceKind.email => _EmailView(
                  load: () =>
                      controller.readEmail(projectId, resource.attachmentId),
                ),
              ProjectResourceKind.document => _Message(
                  icon: Icons.description_outlined,
                  text: l10n.thisKindOfDocumentCanT,
                ),
            },
    );
  }
}

/// Shows a resource through its clear copy (#99), or says it cannot be opened.
final class _Clear extends StatelessWidget {
  const _Clear({required this.resource, required this.builder});

  final ProjectResource resource;
  final Widget Function(String path) builder;

  @override
  Widget build(BuildContext context) {
    return ClearFile(
      attachmentId: resource.attachmentId,
      builder: (BuildContext context, String path) => builder(path),
      unavailableBuilder: (BuildContext context) => _Message(
        icon: Icons.error_outline,
        text: l10n.thisFileCannotBeOpened,
      ),
    );
  }
}

/// Reads a text file and shows it with `builder`.
final class _FileText extends StatelessWidget {
  const _FileText({required this.filePath, required this.builder});

  final String filePath;
  final Widget Function(String text) builder;

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<String>(
      future: File(filePath)
          .readAsBytes()
          .then((List<int> bytes) => utf8.decode(bytes, allowMalformed: true)),
      builder: (BuildContext context, AsyncSnapshot<String> snapshot) =>
          switch (snapshot) {
        AsyncSnapshot<String>(hasData: true, :final String data) =>
          builder(data),
        AsyncSnapshot<String>(hasError: true) => _Message(
            icon: Icons.error_outline,
            text: l10n.thisFileCannotBeRead,
          ),
        _ => const Center(child: CircularProgressIndicator()),
      },
    );
  }
}

/// Shows an e-mail's headers and plain-text body.
final class _EmailView extends StatefulWidget {
  const _EmailView({required this.load});

  final Future<EmailView> Function() load;

  @override
  State<_EmailView> createState() => _EmailViewState();
}

/// Loads the e-mail once.
final class _EmailViewState extends State<_EmailView> {
  late final Future<EmailView> _email = widget.load();

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return FutureBuilder<EmailView>(
      future: _email,
      builder: (BuildContext context, AsyncSnapshot<EmailView> snapshot) {
        if (snapshot.hasError) {
          return _Message(
            icon: Icons.error_outline,
            text: l10n.thisEMailCannotBeRead,
          );
        }
        final EmailView? email = snapshot.data;
        if (email == null) {
          return const Center(child: CircularProgressIndicator());
        }
        String people(List<EmailPerson> people) =>
            people.map((EmailPerson person) => person.label).join(', ');
        return SelectionArea(
          child: ListView(
            padding: const EdgeInsets.all(16),
            children: <Widget>[
              Text(email.subject ?? l10n.noSubject,
                  style: theme.textTheme.titleLarge),
              const SizedBox(height: 12),
              for (final (String label, String value) in <(String, String)>[
                (l10n.from, people(email.from)),
                (l10n.to, people(email.to)),
                (l10n.cc, people(email.cc)),
                if (email.sentAt case final DateTime sentAt)
                  (
                    l10n.sent,
                    MaterialLocalizations.of(context)
                        .formatFullDate(sentAt.toLocal())
                  ),
              ])
                if (value.isNotEmpty)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 4),
                    child: Text.rich(TextSpan(children: <InlineSpan>[
                      TextSpan(
                        text: '$label: ',
                        style: const TextStyle(fontWeight: FontWeight.bold),
                      ),
                      TextSpan(text: value),
                    ])),
                  ),
              if (email.attachmentNames.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Text(
                    l10n.attachments(email.attachmentNames.join(', ')),
                    style: theme.textTheme.bodySmall,
                  ),
                ),
              const Divider(height: 24),
              Text(email.bodyText.trim()),
              if (email.bodyTruncated)
                Padding(
                  padding: const EdgeInsets.only(top: 12),
                  child: Text(l10n.theRestOfThisEMail,
                      style: theme.textTheme.bodySmall),
                ),
            ],
          ),
        );
      },
    );
  }
}

/// A centered icon and sentence.
final class _Message extends StatelessWidget {
  const _Message({required this.icon, required this.text});

  final IconData icon;
  final String text;

  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Icon(icon, size: 48),
              const SizedBox(height: 12),
              Text(text, textAlign: TextAlign.center),
            ],
          ),
        ),
      );
}
