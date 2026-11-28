// Switches between the app's top-level screens with Material 3 adaptive navigation. Meetings is
// home; Record is a button on every main screen (a floating button on phones, the top of the rail
// on desktops) that opens the recorder as a page, and a recording in progress shows as a bar
// that stops it or reopens the recorder from anywhere (#208).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/presentation/keyboard_shortcuts.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/presentation/koalino_logo.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Hosts the Meetings and People (voice gallery) destinations, plus Projects (Issue #85, between
/// them) and Settings (Issue #46) when given their builders, as the app always does. The recorder
/// is not a destination: [openRecorder] opens it as a page (#208).
/// Telemetry (Issue #53) and Devices (Issue #84) are pages of Settings.
///
/// Phones get a bottom `NavigationBar`; windows at least `twoPaneBreakpoint` wide get a
/// `NavigationRail`, with Settings kept at its bottom (#142). Only the selected destination is built, so hidden screens neither poll
/// nor hold stale data (the meeting list reloads each time it is opened).
final class HomeShell extends StatefulWidget {
  /// Creates the shell from one builder per destination.
  ///
  /// Inputs: Builders for the recording, meeting list and voice gallery screens, and optionally
  /// the projects and settings screens, supplied by the composition root (tests pass
  /// placeholders).
  /// `recordingController`, when given, adds the bar shown while a recording runs.
  /// Outputs: The navigation shell, starting on Meetings.
  /// Side effects: None.
  const HomeShell({
    required this.recordingScreenBuilder,
    required this.meetingsScreenBuilder,
    required this.voicesScreenBuilder,
    this.projectsScreenBuilder,
    this.settingsScreenBuilder,
    this.recordingController,
    super.key,
  });

  final WidgetBuilder recordingScreenBuilder;
  final WidgetBuilder meetingsScreenBuilder;
  final WidgetBuilder voicesScreenBuilder;
  final WidgetBuilder? projectsScreenBuilder;
  final WidgetBuilder? settingsScreenBuilder;
  final RecordingController? recordingController;

  /// Opens the recorder as a page from any screen inside the shell, such as an empty meeting
  /// list's button (#204, #208).
  ///
  /// Inputs: a `context` below the shell. Outputs: completes when the recorder page closes, or at
  /// once outside a shell. Side effects: pushes a route.
  static Future<void> openRecorder(BuildContext context) =>
      context.findAncestorStateOfType<_HomeShellState>()?._openRecorder() ??
      Future<void>.value();

  @override
  State<HomeShell> createState() => _HomeShellState();
}

/// Tracks the selected destination.
final class _HomeShellState extends State<HomeShell> {
  int _selectedIndex = 0;

  /// Whether the recorder page is open, so the recording bar hides behind it.
  bool _recorderOpen = false;

  /// Opens the recorder page and waits until it closes.
  Future<void> _openRecorder() async {
    if (_recorderOpen) return;
    setState(() => _recorderOpen = true);
    await Navigator.of(context)
        .push(MaterialPageRoute<void>(builder: widget.recordingScreenBuilder));
    if (mounted) setState(() => _recorderOpen = false);
  }

  /// The destinations shown, in display order, each with the builder of its screen.
  List<
          ({
            IconData icon,
            IconData selectedIcon,
            String label,
            WidgetBuilder builder
          })>
      get _destinations => <({
            IconData icon,
            IconData selectedIcon,
            String label,
            WidgetBuilder builder
          })>[
            (
              icon: Icons.event_note_outlined,
              selectedIcon: Icons.event_note,
              label: l10n.meetings,
              builder: widget.meetingsScreenBuilder,
            ),
            if (widget.projectsScreenBuilder case final WidgetBuilder builder)
              (
                icon: Icons.folder_outlined,
                selectedIcon: Icons.folder,
                label: l10n.projects,
                builder: builder,
              ),
            (
              icon: Icons.people_outline,
              selectedIcon: Icons.people,
              label: l10n.people,
              builder: widget.voicesScreenBuilder,
            ),
            if (widget.settingsScreenBuilder case final WidgetBuilder builder)
              (
                icon: Icons.settings_outlined,
                selectedIcon: Icons.settings,
                label: l10n.settings,
                builder: builder,
              ),
          ];

  void _selectDestination(int index) => setState(() => _selectedIndex = index);

  /// Whether a recording runs or is paused (or is starting or being saved).
  bool get _recording => switch (widget.recordingController?.state) {
        RecordingUiState.recording ||
        RecordingUiState.paused ||
        RecordingUiState.stopping =>
          true,
        _ => false,
      };

  /// The bar shown under the screen while recording and the recorder is closed (#208).
  List<Widget> _recordingBar() => <Widget>[
        if (widget.recordingController case final RecordingController controller
            when _recording && !_recorderOpen)
          RecordingBar(controller: controller, onOpen: _openRecorder),
      ];

  @override
  void initState() {
    super.initState();
    widget.recordingController?.addListener(_onRecordingChanged);
  }

  @override
  void didUpdateWidget(HomeShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.recordingController != widget.recordingController) {
      oldWidget.recordingController?.removeListener(_onRecordingChanged);
      widget.recordingController?.addListener(_onRecordingChanged);
    }
  }

  @override
  void dispose() {
    widget.recordingController?.removeListener(_onRecordingChanged);
    super.dispose();
  }

  void _onRecordingChanged() => setState(() {});

  /// The commands of the Ctrl/Cmd+K palette (#215): Record, every destination, the shortcuts.
  List<PaletteCommand> _paletteCommands() => <PaletteCommand>[
        (
          icon: Icons.mic,
          label: l10n.startRecording,
          shortcut: 'R',
          run: _openRecorder,
        ),
        for (final (int index, destination) in _destinations.indexed)
          (
            icon: destination.icon,
            label: l10n.goToPlace(destination.label),
            shortcut: null,
            run: () => _selectDestination(index),
          ),
        (
          icon: Icons.keyboard_outlined,
          label: l10n.keyboardShortcuts,
          shortcut: '?',
          run: () => showKeyboardShortcuts(context),
        ),
      ];

  @override
  Widget build(BuildContext context) {
    // Power-user keys on every main screen (#215).
    return AppShortcuts(
      bindings: <ShortcutActivator, VoidCallback>{
        commandPaletteActivator: () =>
            showCommandPalette(context, _paletteCommands()),
        const CharacterActivator('r'): _openRecorder,
        const CharacterActivator('?'): () => showKeyboardShortcuts(context),
      },
      child: _layout(context),
    );
  }

  /// The phone or desktop layout around the selected screen.
  Widget _layout(BuildContext context) {
    final destinations = _destinations;
    final int selectedIndex =
        _selectedIndex < destinations.length ? _selectedIndex : 0;
    final Widget selectedScreen = KeyedSubtree(
      key: ValueKey<String>(destinations[selectedIndex].label),
      child: Builder(builder: destinations[selectedIndex].builder),
    );
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        if (constraints.maxWidth < twoPaneBreakpoint) {
          // Record floats over Meetings and People; Projects has its own New project button
          // there, and the settings need none. While recording, the bar takes its place.
          final String selectedLabel = destinations[selectedIndex].label;
          final bool recordButton = !_recording &&
              (selectedLabel == l10n.meetings || selectedLabel == l10n.people);
          return Scaffold(
            body: Column(
              children: <Widget>[
                Expanded(child: selectedScreen),
                ..._recordingBar(),
              ],
            ),
            floatingActionButton: recordButton
                ? FloatingActionButton.extended(
                    heroTag: 'record',
                    onPressed: _openRecorder,
                    icon: const Icon(Icons.mic),
                    label: Text(l10n.record),
                  )
                : null,
            bottomNavigationBar: NavigationBar(
              selectedIndex: selectedIndex,
              onDestinationSelected: _selectDestination,
              destinations: <Widget>[
                for (final destination in destinations)
                  NavigationDestination(
                    icon: Icon(destination.icon),
                    selectedIcon: Icon(destination.selectedIcon),
                    label: destination.label,
                  ),
              ],
            ),
          );
        }
        // Settings stays at the bottom of the rail, apart from the destinations (#142).
        final bool settingsApart = widget.settingsScreenBuilder != null;
        final int railCount =
            settingsApart ? destinations.length - 1 : destinations.length;
        return Scaffold(
          body: Row(
            children: <Widget>[
              SafeArea(
                child: NavigationRail(
                  selectedIndex:
                      selectedIndex < railCount ? selectedIndex : null,
                  onDestinationSelected: _selectDestination,
                  labelType: NavigationRailLabelType.all,
                  // The logo tops the rail, so the app shows its own icon (#172); Record sits
                  // under it, the first thing on every screen (#208).
                  leading: Padding(
                    padding: const EdgeInsets.only(top: 8, bottom: 12),
                    child: Column(
                      children: <Widget>[
                        const KoalinoLogo(size: 40),
                        const SizedBox(height: 16),
                        FloatingActionButton(
                          key: const ValueKey<String>('rail-record'),
                          heroTag: 'record',
                          tooltip: l10n.record,
                          elevation: 0,
                          onPressed: _openRecorder,
                          child: const Icon(Icons.mic),
                        ),
                        const SizedBox(height: 4),
                        Text(l10n.record,
                            style: Theme.of(context).textTheme.labelMedium),
                      ],
                    ),
                  ),
                  trailingAtBottom: true,
                  trailing: settingsApart
                      ? Padding(
                          padding: const EdgeInsets.only(bottom: 16),
                          child: _RailButton(
                            destination: destinations.last,
                            selected: selectedIndex == railCount,
                            onTap: () => _selectDestination(railCount),
                          ),
                        )
                      : null,
                  destinations: <NavigationRailDestination>[
                    for (final destination in destinations.take(railCount))
                      NavigationRailDestination(
                        icon: Icon(destination.icon),
                        selectedIcon: Icon(destination.selectedIcon),
                        label: Text(destination.label),
                      ),
                  ],
                ),
              ),
              const VerticalDivider(width: 1),
              Expanded(
                child: Column(
                  children: <Widget>[
                    Expanded(child: selectedScreen),
                    ..._recordingBar(),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// A destination drawn like the rail's own, for Settings at the rail's bottom (#142).
final class _RailButton extends StatelessWidget {
  const _RailButton(
      {required this.destination, required this.selected, required this.onTap});

  final ({
    IconData icon,
    IconData selectedIcon,
    String label,
    WidgetBuilder builder
  }) destination;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final ColorScheme colors = Theme.of(context).colorScheme;
    final Color foreground =
        selected ? colors.onSecondaryContainer : colors.onSurfaceVariant;
    return Semantics(
      button: true,
      selected: selected,
      child: InkWell(
        key: const ValueKey<String>('rail-settings'),
        onTap: onTap,
        customBorder: const StadiumBorder(),
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 4, horizontal: 8),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              AnimatedContainer(
                duration: const Duration(milliseconds: 150),
                width: 56,
                height: 32,
                decoration: ShapeDecoration(
                  shape: const StadiumBorder(),
                  color: selected ? colors.secondaryContainer : null,
                ),
                child: Icon(
                    selected ? destination.selectedIcon : destination.icon,
                    color: foreground),
              ),
              const SizedBox(height: 4),
              Text(destination.label,
                  style: Theme.of(context).textTheme.labelMedium?.copyWith(
                      color: selected ? colors.onSurface : foreground)),
            ],
          ),
        ),
      ),
    );
  }
}

/// The bar shown while a recording runs and the recorder page is closed (#208): what is being
/// recorded, Pause or Resume, Stop, and a tap anywhere else reopens the recorder.
final class RecordingBar extends StatelessWidget {
  /// Inputs: the app's `controller`; `onOpen` opens the recorder page.
  /// Outputs: the bar. Side effects: pauses, resumes or stops the recording when asked.
  const RecordingBar(
      {required this.controller, required this.onOpen, super.key})
      : super();

  final RecordingController controller;
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context) {
    final ColorScheme colors = Theme.of(context).colorScheme;
    final bool paused = controller.state == RecordingUiState.paused;
    final bool busy = controller.state == RecordingUiState.stopping;
    final String status = paused ? l10n.pausedNow : l10n.recordingNow;
    return Material(
      key: const ValueKey<String>('recording-bar'),
      color: colors.secondaryContainer,
      child: SafeArea(
        top: false,
        child: InkWell(
          onTap: onOpen,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 4, 8, 4),
            child: Row(
              children: <Widget>[
                Icon(paused ? Icons.pause_circle : Icons.fiber_manual_record,
                    color: paused ? colors.onSecondaryContainer : colors.error,
                    size: 18),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        controller.activeTitle ?? status,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context)
                            .textTheme
                            .titleSmall
                            ?.copyWith(color: colors.onSecondaryContainer),
                      ),
                      if (controller.activeTitle != null)
                        Text(status,
                            style: Theme.of(context)
                                .textTheme
                                .bodySmall
                                ?.copyWith(color: colors.onSecondaryContainer)),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: paused ? l10n.resume : l10n.pause,
                  onPressed: busy
                      ? null
                      : () => paused
                          ? controller.resumeRecording()
                          : controller.pauseRecording(),
                  icon: Icon(paused ? Icons.play_arrow : Icons.pause),
                ),
                IconButton(
                  tooltip: l10n.stop,
                  onPressed: busy ? null : controller.stopRecording,
                  icon: const Icon(Icons.stop),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
