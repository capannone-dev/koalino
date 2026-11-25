// Switches between the app's top-level screens with Material 3 adaptive navigation.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/presentation/koalino_logo.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Hosts the Record, Meetings and Voices destinations, plus Projects (Issue #85, between Meetings
/// and Voices) and Settings (Issue #46) when given their builders, as the app always does.
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
              label: l10n.record,
              builder: widget.recordingScreenBuilder,
            ),
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
              icon: Icons.record_voice_over_outlined,
              selectedIcon: Icons.record_voice_over,
              label: l10n.voices,
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
                  // The logo tops the rail, so the app shows its own icon (#172).
                  leading: const Padding(
                    padding: EdgeInsets.only(top: 8, bottom: 12),
                    child: KoalinoLogo(size: 40),
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
              Expanded(child: selectedScreen),
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
