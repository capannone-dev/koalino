// Renders Markdown project resources inside the app (Issue #85) with the pure-Dart `markdown`
// parser: headings, paragraphs, emphasis, code, lists, quotes, rules and tables. Nothing is
// fetched or opened: links show as underlined text and images as their description, so a
// resource from a paired device cannot reach the network.

import 'package:flutter/material.dart';
import 'package:markdown/markdown.dart' as md;

/// Shows Markdown text as Material 3 widgets.
final class MarkdownView extends StatelessWidget {
  /// Creates the view for `markdownText`; `embedded` lays it out as a column for use inside
  /// another scrolling view (a meeting summary, #95) instead of scrolling on its own.
  const MarkdownView({
    required this.markdownText,
    this.embedded = false,
    super.key,
  });

  /// The Markdown source.
  final String markdownText;

  /// Whether it sits inside another scrolling view.
  final bool embedded;

  @override
  Widget build(BuildContext context) {
    final List<md.Node> nodes =
        md.Document(extensionSet: md.ExtensionSet.gitHubFlavored)
            .parseLines(markdownText.replaceAll('\r\n', '\n').split('\n'));
    final List<Widget> blocks = <Widget>[
      for (final md.Node node in nodes) _MarkdownBlock(node: node),
    ];
    if (embedded) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: blocks,
      );
    }
    return SelectionArea(
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: blocks,
      ),
    );
  }
}

/// One block-level node.
final class _MarkdownBlock extends StatelessWidget {
  const _MarkdownBlock({required this.node, this.listMarker});

  final md.Node node;

  /// The bullet or number shown before a list item.
  final String? listMarker;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final md.Node block = node;
    if (block is md.Text) {
      return _paragraph(context, <md.Node>[block], theme.textTheme.bodyLarge);
    }
    if (block is! md.Element) {
      return const SizedBox.shrink();
    }
    final List<md.Node> children = block.children ?? const <md.Node>[];
    return switch (block.tag) {
      'h1' => _paragraph(context, children, theme.textTheme.headlineMedium),
      'h2' => _paragraph(context, children, theme.textTheme.headlineSmall),
      'h3' => _paragraph(context, children, theme.textTheme.titleLarge),
      'h4' ||
      'h5' ||
      'h6' =>
        _paragraph(context, children, theme.textTheme.titleMedium),
      'p' => _paragraph(context, children, theme.textTheme.bodyLarge),
      'hr' => const Divider(height: 32),
      'pre' => Container(
          width: double.infinity,
          margin: const EdgeInsets.symmetric(vertical: 8),
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: theme.colorScheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(8),
          ),
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Text(
              block.textContent.trimRight(),
              style:
                  theme.textTheme.bodyMedium?.copyWith(fontFamily: 'monospace'),
            ),
          ),
        ),
      'blockquote' => Container(
          margin: const EdgeInsets.symmetric(vertical: 8),
          padding: const EdgeInsets.only(left: 12),
          decoration: BoxDecoration(
            border: Border(
              left: BorderSide(color: theme.colorScheme.outline, width: 3),
            ),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              for (final md.Node child in children) _MarkdownBlock(node: child),
            ],
          ),
        ),
      'ul' || 'ol' => Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            for (final (int index, md.Node item) in children.indexed)
              _MarkdownBlock(
                node: item,
                listMarker: block.tag == 'ol'
                    ? '${(int.tryParse(block.attributes['start'] ?? '') ?? 1) + index}.'
                    : '•',
              ),
          ],
        ),
      'li' => Padding(
          padding: const EdgeInsets.symmetric(vertical: 2),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              SizedBox(
                width: 28,
                child:
                    Text(listMarker ?? '•', style: theme.textTheme.bodyLarge),
              ),
              Expanded(
                child: children.any((md.Node child) =>
                        child is md.Element &&
                        const <String>{'p', 'ul', 'ol', 'pre', 'blockquote'}
                            .contains(child.tag))
                    ? Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          for (final md.Node child in children)
                            _MarkdownBlock(node: child),
                        ],
                      )
                    : _paragraph(context, children, theme.textTheme.bodyLarge,
                        spacing: 0),
              ),
            ],
          ),
        ),
      'table' => _table(context, block),
      _ => _paragraph(context, children, theme.textTheme.bodyLarge),
    };
  }

  /// A paragraph of inline content in `style`.
  Widget _paragraph(
          BuildContext context, List<md.Node> inline, TextStyle? style,
          {double spacing = 6}) =>
      Padding(
        padding: EdgeInsets.symmetric(vertical: spacing),
        child: Text.rich(
          TextSpan(
            style: style,
            children: <InlineSpan>[
              for (final md.Node child in inline) _inlineSpan(context, child),
            ],
          ),
        ),
      );

  /// A GitHub-style table, scrolled sideways when wide.
  Widget _table(BuildContext context, md.Element table) {
    final List<md.Element> rows = <md.Element>[
      for (final md.Node section in table.children ?? const <md.Node>[])
        if (section is md.Element)
          for (final md.Node row in section.children ?? const <md.Node>[])
            if (row is md.Element && row.tag == 'tr') row,
    ];
    final ThemeData theme = Theme.of(context);
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Table(
        defaultColumnWidth: const IntrinsicColumnWidth(),
        border: TableBorder.all(color: theme.colorScheme.outlineVariant),
        children: <TableRow>[
          for (final md.Element row in rows)
            TableRow(
              children: <Widget>[
                for (final md.Node cell in row.children ?? const <md.Node>[])
                  Padding(
                    padding: const EdgeInsets.all(8),
                    child: Text.rich(TextSpan(
                      style: cell is md.Element && cell.tag == 'th'
                          ? theme.textTheme.bodyMedium
                              ?.copyWith(fontWeight: FontWeight.bold)
                          : theme.textTheme.bodyMedium,
                      children: <InlineSpan>[
                        if (cell is md.Element)
                          for (final md.Node child
                              in cell.children ?? const <md.Node>[])
                            _inlineSpan(context, child),
                      ],
                    )),
                  ),
              ],
            ),
        ],
      ),
    );
  }

  /// One inline node as a span.
  InlineSpan _inlineSpan(BuildContext context, md.Node inline) {
    if (inline is md.Text) {
      return TextSpan(text: inline.text);
    }
    if (inline is! md.Element) {
      return const TextSpan();
    }
    final ThemeData theme = Theme.of(context);
    final List<InlineSpan> children = <InlineSpan>[
      for (final md.Node child in inline.children ?? const <md.Node>[])
        _inlineSpan(context, child),
    ];
    return switch (inline.tag) {
      'strong' => TextSpan(
          style: const TextStyle(fontWeight: FontWeight.bold),
          children: children),
      'em' => TextSpan(
          style: const TextStyle(fontStyle: FontStyle.italic),
          children: children),
      'del' => TextSpan(
          style: const TextStyle(decoration: TextDecoration.lineThrough),
          children: children),
      'code' => TextSpan(
          text: inline.textContent,
          style: TextStyle(
            fontFamily: 'monospace',
            backgroundColor: theme.colorScheme.surfaceContainerHighest,
          ),
        ),
      'a' => TextSpan(
          style: TextStyle(
            color: theme.colorScheme.primary,
            decoration: TextDecoration.underline,
          ),
          children: children,
        ),
      'img' => TextSpan(
          text: '[${inline.attributes['alt'] ?? 'image'}]',
          style: const TextStyle(fontStyle: FontStyle.italic),
        ),
      'br' => const TextSpan(text: '\n'),
      _ => TextSpan(children: children),
    };
  }
}
