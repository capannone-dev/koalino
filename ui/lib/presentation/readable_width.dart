// Keeps long text at a readable width while its scroll view fills the whole pane (#142).
//
// A list built inside a centred, width-limited box only scrolls with the mouse wheel over that
// box. These helpers let the list take the full pane and put the empty sides into its padding,
// so the wheel works anywhere across the pane and the scroll bar sits at the pane's edge.

import 'package:flutter/widgets.dart';

/// The padding that centres content at most `maxWidth` wide in a pane `paneWidth` wide.
///
/// Inputs: The pane's width, the content's largest width (padding included, as the boxes this
/// replaces measured it) and the padding kept on every side. Outputs: Padding whose sides grow
/// with the pane so the content stays centred and no wider than `maxWidth`. Side effects: None.
EdgeInsets readablePadding(double paneWidth,
    {required double maxWidth, double inset = 24}) {
  final double side =
      paneWidth > maxWidth ? (paneWidth - maxWidth) / 2 + inset : inset;
  return EdgeInsets.fromLTRB(side, inset, side, inset);
}

/// Builds `builder` with the padding that keeps its content readable in the available width.
///
/// Inputs: The content's largest width, the padding kept on every side and the builder of the
/// full-width scroll view, given that padding. Outputs: The scroll view. Side effects: None.
final class ReadableWidth extends StatelessWidget {
  /// Creates the wrapper.
  const ReadableWidth(
      {required this.maxWidth,
      required this.builder,
      this.inset = 24,
      super.key});

  /// The content's largest width, padding included.
  final double maxWidth;

  /// The padding kept on every side.
  final double inset;

  /// Builds the full-width scroll view with the given padding.
  final Widget Function(BuildContext context, EdgeInsets padding) builder;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
        builder: (BuildContext context, BoxConstraints constraints) => builder(
            context,
            readablePadding(constraints.maxWidth,
                maxWidth: maxWidth, inset: inset)),
      );
}
