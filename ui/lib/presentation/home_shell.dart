// Switches between the app's top-level screens with Material 3 adaptive navigation.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';

/// Hosts the Record, Meetings, Voices, and Devices destinations, plus Settings when given a builder
/// (the app always gives one; Issue #46). Telemetry is a page of Settings (Issue #53).
///
/// Phones get a bottom `NavigationBar`; windows at least `twoPaneBreakpoint` wide get a
/// `NavigationRail`. Only the selected destination is built, so hidden screens neither poll
/// nor hold stale data (the meeting list reloads each time it is opened).
final class HomeShell extends StatefulWidget {
  /// Creates the shell from one builder per destination.
  ///
  /// Inputs: Builders for the recording, meeting list, voice gallery, and devices screens, and
  /// optionally the settings screen, supplied by the composition root (tests pass
  /// placeholders).
  /// Outputs: The navigation shell, starting on Record.
  /// Side effects: None.
  const HomeShell({
    required this.recordingScreenBuilder,
    required this.meetingsScreenBuilder,
    required this.voicesScreenBuilder,
    required this.devicesScreenBuilder,
    this.settingsScreenBuilder,
    super.key,
  });

  final WidgetBuilder recordingScreenBuilder;
  final WidgetBuilder meetingsScreenBuilder;
  final WidgetBuilder voicesScreenBuilder;
  final WidgetBuilder devicesScreenBuilder;
  final WidgetBuilder? settingsScreenBuilder;

  @override
  State<HomeShell> createState() => _HomeShellState();
}

/// Tracks the selected destination.
final class _HomeShellState extends State<HomeShell> {
  int _selectedIndex = 0;

  /// The destinations always shown, in display order.
  static const List<({IconData icon, IconData selectedIcon, String label})>
      _mainDestinations =
      <({IconData icon, IconData selectedIcon, String label})>[
    (icon: Icons.mic_none, selectedIcon: Icons.mic, label: 'Record'),
    (
      icon: Icons.event_note_outlined,
      selectedIcon: Icons.event_note,
      label: 'Meetings'
    ),
    (
      icon: Icons.record_voice_over_outlined,
      selectedIcon: Icons.record_voice_over,
      label: 'Voices'
    ),
    (
      icon: Icons.devices_outlined,
      selectedIcon: Icons.devices,
      label: 'Devices'
    ),
  ];

  /// The Settings destination, last, shown when the shell has a settings builder.
  static const ({
    IconData icon,
    IconData selectedIcon,
    String label
  }) _settingsDestination = (
    icon: Icons.settings_outlined,
    selectedIcon: Icons.settings,
    label: 'Settings'
  );

  /// The destinations shown, in display order.
  List<({IconData icon, IconData selectedIcon, String label})>
      get _destinations =>
          <({IconData icon, IconData selectedIcon, String label})>[
            ..._mainDestinations,
            if (widget.settingsScreenBuilder != null) _settingsDestination,
          ];

  void _selectDestination(int index) => setState(() => _selectedIndex = index);

  @override
  Widget build(BuildContext context) {
    final Widget selectedScreen = KeyedSubtree(
      key: ValueKey<int>(_selectedIndex),
      child: Builder(
        builder: switch (_selectedIndex) {
          1 => widget.meetingsScreenBuilder,
          2 => widget.voicesScreenBuilder,
          3 => widget.devicesScreenBuilder,
          4 => widget.settingsScreenBuilder ?? widget.recordingScreenBuilder,
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
