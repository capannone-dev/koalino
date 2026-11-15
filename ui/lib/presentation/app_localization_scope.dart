// Puts the language MaterialApp resolved into effect for the whole app (Issue #124). Screens and
// controllers read their text from `l10n` without a BuildContext, so when the language changes
// this scope swaps the text and rebuilds every widget once, keeping their state.

import 'package:flutter/widgets.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Sits in `MaterialApp.builder`, under the app's Localizations.
final class AppLocalizationScope extends StatelessWidget {
  /// Creates the scope around the navigator.
  ///
  /// Inputs: The navigator (or what wraps it). Outputs: A widget showing `child` unchanged.
  /// Side effects: None until built.
  const AppLocalizationScope({required this.child, super.key});

  /// What MaterialApp builds below.
  final Widget child;

  /// Side effects: Makes the resolved locale's text the one `l10n` returns; when that changes the
  /// language, marks every element below for rebuild after this frame.
  @override
  Widget build(BuildContext context) {
    final AppLocalizations localizations = AppLocalizations.of(context);
    if (useAppLocalizations(localizations)) {
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        if (context.mounted) {
          _rebuildAll(context);
        }
      });
    }
    return child;
  }

  /// Marks every element below `context` dirty, so text read from `l10n` is read again.
  static void _rebuildAll(BuildContext context) {
    void rebuild(Element element) {
      element.markNeedsBuild();
      element.visitChildren(rebuild);
    }

    (context as Element).visitChildren(rebuild);
  }
}
