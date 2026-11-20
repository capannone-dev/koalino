// A column of the desktop layout whose width the owner sets by dragging its right edge (#142).
//
// `ResizablePane` shows its child at a width between `minWidth` and `maxWidth`, followed by a
// thin divider that can be dragged with the mouse, moved with the arrow keys when focused, or
// double-clicked to go back to the default width. Each pane has an `id`; its width is kept in
// `paneWidthStore`, a JSON file in the app's support folder on desktops (`ui-layout.json`), so
// the layout survives restarts. Widths are layout preferences only: nothing about meetings is
// stored, and nothing is synced.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Where pane widths are remembered.
abstract interface class PaneWidthStore {
  /// The width saved for `id`, or null when none was.
  ///
  /// Side effects: May read the store's file the first time.
  Future<double?> read(String id);

  /// Saves `width` for `id`. Side effects: May write the store's file.
  Future<void> write(String id, double width);
}

/// Keeps widths in memory only: the default, and what tests use.
final class MemoryPaneWidthStore implements PaneWidthStore {
  /// The saved widths, by pane ID.
  final Map<String, double> widths = <String, double>{};

  @override
  Future<double?> read(String id) async => widths[id];

  @override
  Future<void> write(String id, double width) async => widths[id] = width;
}

/// Keeps widths in a small JSON file, read once and rewritten on each change.
final class FilePaneWidthStore implements PaneWidthStore {
  /// Creates the store over `file`; the file need not exist yet.
  FilePaneWidthStore(this.file);

  /// The JSON file, `{"<pane id>": <width>}`.
  final File file;

  Map<String, double>? _widths;

  Future<Map<String, double>> _load() async {
    if (_widths case final Map<String, double> widths) {
      return widths;
    }
    final Map<String, double> loaded = <String, double>{};
    try {
      final Object? decoded = jsonDecode(await file.readAsString());
      if (decoded is Map<String, Object?>) {
        for (final MapEntry<String, Object?> entry in decoded.entries) {
          if (entry.value case final num width) {
            loaded[entry.key] = width.toDouble();
          }
        }
      }
    } on Object {
      // A missing or damaged file means the default widths.
    }
    return _widths = loaded;
  }

  @override
  Future<double?> read(String id) async => (await _load())[id];

  @override
  Future<void> write(String id, double width) async {
    final Map<String, double> widths = await _load();
    widths[id] = width;
    try {
      await file.parent.create(recursive: true);
      final File partial = File('${file.path}.tmp');
      await partial.writeAsString(jsonEncode(widths));
      await partial.rename(file.path);
    } on Object {
      // The width still applies until the app closes.
    }
  }
}

/// The store every pane uses; `main` points it at a file on desktops.
PaneWidthStore paneWidthStore = MemoryPaneWidthStore();

/// How far one arrow key press moves a divider.
const double _keyboardStep = 16;

/// Width of the divider's grab area; the drawn line stays 1 px.
const double _handleWidth = 8;

/// A column with a draggable right edge.
final class ResizablePane extends StatefulWidget {
  /// Creates the pane.
  ///
  /// Inputs: `id` names the saved width (and the keys `pane-<id>` and `resize-<id>`); the
  /// default, smallest and largest widths (the largest is usually what leaves the next column
  /// room, so it may shrink with the window); and the child. Side effects: Reads the saved width
  /// when first shown.
  const ResizablePane({
    required this.id,
    required this.defaultWidth,
    required this.child,
    this.minWidth = 220,
    this.maxWidth = 720,
    super.key,
  });

  final String id;
  final double defaultWidth;
  final double minWidth;
  final double maxWidth;
  final Widget child;

  @override
  State<ResizablePane> createState() => _ResizablePaneState();
}

final class _ResizablePaneState extends State<ResizablePane> {
  double? _width;

  @override
  void initState() {
    super.initState();
    unawaited(paneWidthStore.read(widget.id).then((double? saved) {
      if (saved != null && mounted && _width == null) {
        setState(() => _width = saved);
      }
    }));
  }

  /// The width shown: the chosen one, kept between the bounds.
  double get _shownWidth {
    final double largest =
        widget.maxWidth < widget.minWidth ? widget.minWidth : widget.maxWidth;
    return (_width ?? widget.defaultWidth).clamp(widget.minWidth, largest);
  }

  void _resize(double width, {bool save = true}) {
    setState(() => _width = width);
    final double shown = _shownWidth;
    if (save) {
      unawaited(paneWidthStore.write(widget.id, shown));
    }
  }

  @override
  Widget build(BuildContext context) {
    final double width = _shownWidth;
    final ColorScheme colors = Theme.of(context).colorScheme;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SizedBox(
          key: ValueKey<String>('pane-${widget.id}'),
          width: width,
          child: widget.child,
        ),
        Semantics(
          label: l10n.columnWidth,
          value: '${width.round()}',
          increasedValue: '${(width + _keyboardStep).round()}',
          decreasedValue: '${(width - _keyboardStep).round()}',
          onIncrease: () => _resize(width + _keyboardStep),
          onDecrease: () => _resize(width - _keyboardStep),
          child: Focus(
            onKeyEvent: (FocusNode node, KeyEvent event) {
              if (event is KeyUpEvent) {
                return KeyEventResult.ignored;
              }
              if (event.logicalKey == LogicalKeyboardKey.arrowLeft) {
                _resize(width - _keyboardStep);
                return KeyEventResult.handled;
              }
              if (event.logicalKey == LogicalKeyboardKey.arrowRight) {
                _resize(width + _keyboardStep);
                return KeyEventResult.handled;
              }
              return KeyEventResult.ignored;
            },
            child: Builder(
              builder: (BuildContext context) => MouseRegion(
                cursor: SystemMouseCursors.resizeColumn,
                child: GestureDetector(
                  key: ValueKey<String>('resize-${widget.id}'),
                  behavior: HitTestBehavior.opaque,
                  // Count the movement from the press, so the start threshold is not lost.
                  dragStartBehavior: DragStartBehavior.down,
                  onHorizontalDragStart: (_) =>
                      Focus.of(context).requestFocus(),
                  onHorizontalDragUpdate: (DragUpdateDetails details) =>
                      _resize(_shownWidth + details.delta.dx, save: false),
                  onHorizontalDragEnd: (_) => _resize(_shownWidth),
                  onDoubleTap: () => _resize(widget.defaultWidth),
                  child: SizedBox(
                    width: _handleWidth,
                    child: Center(
                      child: Container(
                        width: Focus.of(context).hasFocus ? 3 : 1,
                        color: Focus.of(context).hasFocus
                            ? colors.primary
                            : colors.outlineVariant,
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
