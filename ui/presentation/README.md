<!-- This directory contains Material 3 widgets that render controller state only. -->

# Presentation

Widgets in this directory receive controllers or immutable state from the controller layer. They
must not call `dart:ffi`, platform channels, or Rust bindings directly.

The current widgets live in `lib/presentation/`, including the Issue #36 navigation shell and
the meeting list and detail screens.
