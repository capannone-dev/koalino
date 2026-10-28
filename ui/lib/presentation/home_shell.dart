// Switches between the app's top-level screens with Material 3 adaptive navigation.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';

/// Hosts the Record, Meetings, Telemetry, and Devices destinations.
///
/// Phones get a bottom `NavigationBar`; windows at least `twoPaneBreakpoint` wide get a
/// `NavigationRail`. Only the selected destination is built, so hidden screens neither poll
/// nor hold stale data (the meeting list reloads each time it is opened).
final class HomeShell extends StatefulWidget {
  /// Creates the shell from one builder per destination.
  ///
  /// Inputs: Builders for the recording, meeting list, telemetry, and devices screens, supplied by the
  /// composition root (tests pass placeholders).
  /// Outputs: The navigation shell, starting on Record.
  /// Side effects: None.
  const HomeShell({
    required this.recordingScreenBuilder,
    required this.meetingsScreenBuilder,
    required this.telemetryScreenBuilder,
    required this.devicesScreenBuilder,
    super.key,
  });

  final WidgetBuilder recordingScreenBuilder;
  final WidgetBuilder meetingsScreenBuilder;
  final WidgetBuilder telemetryScreenBuilder;
  final WidgetBuilder devicesScreenBuilder;

  @override
  State<HomeShell> createState() => _HomeShellState();
}

/// Tracks the selected destination.
final class _HomeShellState extends State<HomeShell> {
  int _selectedIndex = 0;

  /// The destinations, in display order.
  static const List<({IconData icon, IconData selectedIcon, String label})>
      _destinations = <({IconData icon, IconData selectedIcon, String label})>[
    (icon: Icons.mic_none, selectedIcon: Icons.mic, label: 'Record'),
    (
      icon: Icons.event_note_outlined,
      selectedIcon: Icons.event_note,
      label: 'Meetings'
    ),
    (
      icon: Icons.insights_outlined,
      selectedIcon: Icons.insights,
      label: 'Telemetry'
    ),
    (
      icon: Icons.devices_outlined,
      selectedIcon: Icons.devices,
      label: 'Devices'
    ),
  ];

  void _selectDestination(int index) => setState(() => _selectedIndex = index);

  @override
  Widget build(BuildContext context) {
    final Widget selectedScreen = KeyedSubtree(
      key: ValueKey<int>(_selectedIndex),
      child: Builder(
        builder: switch (_selectedIndex) {
          1 => widget.meetingsScreenBuilder,
          2 => widget.telemetryScreenBuilder,
          3 => widget.devicesScreenBuilder,
          _ => widget.recordingScreenBuilder,
        },
      ),
    );
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        if (constraints.maxWidth < twoPaneBreakpoint) {
          return Scaffold(
            body: selectedScreen,
            bottomNavigationBar: NavigationBar(
              selectedIndex: _selectedIndex,
              onDestinationSelected: _selectDestination,
              destinations: <Widget>[
                for (final destination in _destinations)
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
                  selectedIndex: _selectedIndex,
                  onDestinationSelected: _selectDestination,
                  labelType: NavigationRailLabelType.all,
                  destinations: <NavigationRailDestination>[
                    for (final destination in _destinations)
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
