// Shows a meeting's photos and screenshots full screen, one at a time (#144).
//
// The left and right arrow keys, the on-screen arrows, and a sideways swipe move between the
// photos; Esc, the back button, and a swipe up or down close the viewer, as photo apps do.
// Pinching or scrolling zooms; while a photo is zoomed in, a swipe pans it instead of turning the
// page or closing. The pictures themselves come from `photoBuilder`, so the gallery knows nothing
// of sealed files.

import 'package:flutter/gestures.dart' show PointerDeviceKind;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// How far a vertical drag must go to close the viewer, in logical pixels.
const double _dismissDistance = 120;

/// How fast a vertical fling must be to close the viewer, in logical pixels per second.
const double _dismissVelocity = 800;

/// Scale above which a photo counts as zoomed in.
const double _zoomedScale = 1.01;

/// The full-screen viewer of several photos.
final class PhotoGallery extends StatefulWidget {
  /// Creates the viewer.
  ///
  /// Inputs: How many photos there are (at least one), which one to show first (clamped to the
  /// range), each photo's title (its file name), and a builder for each picture.
  /// Outputs: A full-screen page. Side effects: None until the user acts (then it may pop the
  /// route).
  const PhotoGallery({
    required this.count,
    required this.initialIndex,
    required this.titleOf,
    required this.photoBuilder,
    super.key,
  });

  /// How many photos the gallery shows.
  final int count;

  /// The photo shown first.
  final int initialIndex;

  /// The title of the photo at an index.
  final String Function(int index) titleOf;

  /// Builds the picture at an index.
  final Widget Function(BuildContext context, int index) photoBuilder;

  @override
  State<PhotoGallery> createState() => _PhotoGalleryState();
}

/// Holds the current page, the zoom of each photo, and an ongoing drag to close.
final class _PhotoGalleryState extends State<PhotoGallery> {
  late int _index = widget.initialIndex.clamp(0, widget.count - 1);
  late final PageController _pages = PageController(initialPage: _index);

  /// One zoom per page, so a photo keeps its zoom while it is on screen.
  final Map<int, TransformationController> _zooms =
      <int, TransformationController>{};

  /// Whether the current photo is zoomed in: swipes then pan it.
  bool _zoomed = false;

  /// The page an arrow is moving to, so a quick second press counts from it.
  int? _heading;

  /// How far the photo has been dragged up or down towards closing.
  double _dismissOffset = 0;

  @override
  void dispose() {
    _pages.dispose();
    for (final TransformationController zoom in _zooms.values) {
      zoom.dispose();
    }
    super.dispose();
  }

  /// Returns the zoom of the photo at `index`, creating it on first use.
  TransformationController _zoomOf(int index) =>
      _zooms.putIfAbsent(index, TransformationController.new);

  /// Shows the photo `step` places away, when there is one.
  ///
  /// Side effects: Animates the page view; resets the zoom of the photo left behind.
  void _go(int step) {
    final int target = (_heading ?? _index) + step;
    if (target < 0 || target >= widget.count) {
      return;
    }
    _heading = target;
    _pages
        .animateToPage(target,
            duration: const Duration(milliseconds: 220), curve: Curves.easeOut)
        .whenComplete(() {
      if (_heading == target) {
        _heading = null;
      }
    });
  }

  /// Records the new page and forgets the zoom of the others.
  void _pageChanged(int index) {
    for (final MapEntry<int, TransformationController> zoom in _zooms.entries) {
      if (zoom.key != index) {
        zoom.value.value = Matrix4.identity();
      }
    }
    setState(() {
      _index = index;
      _zoomed = false;
    });
  }

  /// Re-reads whether the current photo is zoomed in after a pinch or scroll.
  void _zoomChanged() {
    final bool zoomed =
        _zoomOf(_index).value.getMaxScaleOnAxis() > _zoomedScale;
    if (zoomed != _zoomed) {
      setState(() => _zoomed = zoomed);
    }
  }

  /// Closes the viewer. Side effects: Pops the route when it can.
  void _close() => Navigator.of(context).maybePop();

  /// Closes the viewer after a long or fast vertical drag; otherwise puts the photo back.
  void _dragEnded(DragEndDetails details) {
    final double velocity = details.primaryVelocity ?? 0;
    if (_dismissOffset.abs() > _dismissDistance ||
        velocity.abs() > _dismissVelocity) {
      _close();
      return;
    }
    setState(() => _dismissOffset = 0);
  }

  @override
  Widget build(BuildContext context) {
    final MaterialLocalizations words = MaterialLocalizations.of(context);
    final bool several = widget.count > 1;
    final String title = widget.titleOf(_index);
    // Fades the black backdrop as the photo is dragged away.
    final double backdrop =
        (1 - _dismissOffset.abs() / (_dismissDistance * 3)).clamp(0.3, 1.0);
    return CallbackShortcuts(
      bindings: <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.arrowRight): () => _go(1),
        const SingleActivator(LogicalKeyboardKey.arrowLeft): () => _go(-1),
        const SingleActivator(LogicalKeyboardKey.escape): _close,
      },
      child: Focus(
        autofocus: true,
        child: Scaffold(
          backgroundColor: Colors.black.withValues(alpha: backdrop),
          appBar: AppBar(
            title: Text(
                several ? '$title · ${_index + 1} / ${widget.count}' : title),
          ),
          body: Stack(
            children: <Widget>[
              GestureDetector(
                // A zoomed photo pans; otherwise a vertical drag closes the viewer.
                onVerticalDragUpdate: _zoomed
                    ? null
                    : (DragUpdateDetails details) => setState(
                        () => _dismissOffset += details.primaryDelta ?? 0),
                onVerticalDragEnd: _zoomed ? null : _dragEnded,
                child: Transform.translate(
                  offset: Offset(0, _dismissOffset),
                  child: ScrollConfiguration(
                    // Touch screens and touchpads swipe between photos; a mouse uses the arrows.
                    behavior: ScrollConfiguration.of(context).copyWith(
                      dragDevices: <PointerDeviceKind>{
                        PointerDeviceKind.touch,
                        PointerDeviceKind.stylus,
                        PointerDeviceKind.invertedStylus,
                        PointerDeviceKind.trackpad,
                      },
                    ),
                    child: PageView.builder(
                      controller: _pages,
                      itemCount: widget.count,
                      onPageChanged: _pageChanged,
                      physics: _zoomed
                          ? const NeverScrollableScrollPhysics()
                          : const PageScrollPhysics(),
                      itemBuilder: (BuildContext context, int index) =>
                          InteractiveViewer(
                        transformationController: _zoomOf(index),
                        maxScale: 5,
                        panEnabled: _zoomed,
                        onInteractionEnd: (_) => _zoomChanged(),
                        child:
                            Center(child: widget.photoBuilder(context, index)),
                      ),
                    ),
                  ),
                ),
              ),
              if (several && _index > 0)
                _PageArrow(
                  alignment: Alignment.centerLeft,
                  icon: Icons.chevron_left,
                  tooltip: words.previousPageTooltip,
                  onPressed: () => _go(-1),
                ),
              if (several && _index < widget.count - 1)
                _PageArrow(
                  alignment: Alignment.centerRight,
                  icon: Icons.chevron_right,
                  tooltip: words.nextPageTooltip,
                  onPressed: () => _go(1),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A round arrow at one side of the viewer, for a mouse or a tap.
final class _PageArrow extends StatelessWidget {
  const _PageArrow({
    required this.alignment,
    required this.icon,
    required this.tooltip,
    required this.onPressed,
  });

  final Alignment alignment;
  final IconData icon;
  final String tooltip;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: alignment,
      child: Padding(
        padding: const EdgeInsets.all(8),
        // Keeps keyboard focus on the gallery, so the arrow keys go on working after a click.
        child: ExcludeFocus(
          child: IconButton.filledTonal(
            icon: Icon(icon),
            tooltip: tooltip,
            onPressed: onPressed,
          ),
        ),
      ),
    );
  }
}
