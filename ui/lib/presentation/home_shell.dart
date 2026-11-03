// Switches between the app's top-level screens with Material 3 adaptive navigation.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';

/// Hosts the Record, Meetings and Voices destinations, plus Projects (Issue #85, between Meetings
/// and Voices) and Settings (Issue #46) when given their builders, as the app always does.
/// Telemetry (Issue #53) and Devices (Issue #84) are pages of Settings.
///
/// Phones get a bottom `NavigationBar`; windows at least `twoPaneBreakpoint` wide get a
/// `NavigationRail`. Only the selected destination is built, so hidden screens neither poll
/// nor hold stale data (the meeting list reloads each time it is opened).
final class HomeShell extends StatefulWidget {
  /// Creates the shell from one builder per destination.
  ///
  /// Inputs: Builders for the recording, meeting list and voice gallery screens, and optionally
  /// the projects and settings screens, supplied by the composition root (tests pass
  /// placeholders).
  /// Outputs: The navigation shell, starting on Record.
  /// Side effects: None.
  const HomeShell({
    required this.recordingScreenBuilder,
    required this.meetingsScreenBuilder,
    required this.voicesScreenBuilder,
    this.projectsScreenBuilder,
    this.settingsScreenBuilder,
    super.key,
  });

  final WidgetBuilder recordingScreenBuilder;
  final WidgetBuilder meetingsScreenBuilder;
  final WidgetBuilder voicesScreenBuilder;
  final WidgetBuilder? projectsScreenBuilder;
  final WidgetBuilder? settingsScreenBuilder;

  @override
  State<HomeShell> createState() => _HomeShellState();
}

/// Tracks the selected destination.
final class _HomeShellState extends State<HomeShell> {
  int _selectedIndex = 0;

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
              icon: Icons.mic_none,
              selectedIcon: Icons.mic,
              label: 'Record',
              builder: widget.recordingScreenBuilder,
            ),
            (
              icon: Icons.event_note_outlined,
              selectedIcon: Icons.event_note,
              label: 'Meetings',
              builder: widget.meetingsScreenBuilder,
            ),
            if (widget.projectsScreenBuilder case final WidgetBuilder builder)
              (
                icon: Icons.folder_outlined,
                selectedIcon: Icons.folder,
                label: 'Projects',
                builder: builder,
              ),
            (
              icon: Icons.record_voice_over_outlined,
              selectedIcon: Icons.record_voice_over,
              label: 'Voices',
              builder: widget.voicesScreenBuilder,
            ),
            if (widget.settingsScreenBuilder case final WidgetBuilder builder)
              (
                icon: Icons.settings_outlined,
                selectedIcon: Icons.settings,
                label: 'Settings',
                builder: builder,
              ),
          ];

  void _selectDestination(int index) => setState(() => _selectedIndex = index);

  @override
  Widget build(BuildContext context) {
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
          return Scaffold(
            body: selectedScreen,
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
        return Scaffold(
          body: Row(
            children: <Widget>[
              SafeArea(
                child: NavigationRail(
                  selectedIndex: selectedIndex,
                  onDestinationSelected: _selectDestination,
                  labelType: NavigationRailLabelType.all,
                  destinations: <NavigationRailDestination>[
                    for (final destination in destinations)
                      NavigationRailDestination(
                        icon: Icon(destination.icon),
                        selectedIcon: Icon(destination.selectedIcon),
                        label: Text(destination.label),
                      ),
                  ],
                ),
              ),
              const VerticalDivider(width: 1),
              Expanded(child: selectedScreen),
            ],
          ),
        );
      },
    );
  }
}
