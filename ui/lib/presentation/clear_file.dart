// Shows a sealed attachment through its clear copy (#99): asks for the copy once, shows a small
// progress indicator while the core decrypts it, then builds the view from the copy's path.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/clear_copy_bridge.dart';

/// Builds a view of one attachment from its clear copy.
final class ClearFile extends StatefulWidget {
  /// Creates the view.
  ///
  /// Inputs: The attachment, the view to build from the copy's path, and what to show when there
  /// is no copy (the file has not arrived or cannot be read). Outputs: A widget.
  /// Side effects: Asks `clearCopyResolver` for the copy once.
  const ClearFile({
    required this.attachmentId,
    required this.builder,
    required this.unavailableBuilder,
    super.key,
  });

  final String attachmentId;
  final Widget Function(BuildContext context, String path) builder;
  final WidgetBuilder unavailableBuilder;

  @override
  State<ClearFile> createState() => _ClearFileState();
}

/// Holds the one request for the copy.
final class _ClearFileState extends State<ClearFile> {
  late Future<String> _path = clearCopyResolver(widget.attachmentId);

  @override
  void didUpdateWidget(ClearFile oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.attachmentId != widget.attachmentId) {
      _path = clearCopyResolver(widget.attachmentId);
    }
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<String>(
      future: _path,
      builder: (BuildContext context, AsyncSnapshot<String> snapshot) =>
          switch (snapshot) {
        AsyncSnapshot<String>(hasData: true, :final String data) =>
          widget.builder(context, data),
        AsyncSnapshot<String>(hasError: true) =>
          widget.unavailableBuilder(context),
        _ => const Center(
            child: Padding(
              padding: EdgeInsets.all(16),
              child: CircularProgressIndicator(),
            ),
          ),
      },
    );
  }
}
