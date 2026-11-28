// The first-run flow (#202): at most three short screens before the app, on first launch only.
// What Koalino does and its privacy promise; the microphone (and, on Android, notifications)
// asked with a reason, on systems that ask; and the transcription language with the recommended
// model. Every screen can be skipped, and the flow is never shown again once skipped or finished.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/transcription_language_controller.dart';
import 'package:note_taker_ui/infrastructure/microphone_permission.dart';
import 'package:note_taker_ui/infrastructure/seen_tips_store.dart';
import 'package:note_taker_ui/l10n/l10n.dart';
import 'package:note_taker_ui/presentation/koalino_logo.dart';

/// The name the flow is remembered under in the seen-tips store.
const String firstRunTipName = 'first-run';

/// The flow's Skip button, for tests.
const Key firstRunSkipKey = ValueKey<String>('first-run-skip');

/// The flow's Next / Start button, for tests.
const Key firstRunNextKey = ValueKey<String>('first-run-next');

/// Shows the first-run flow over `child` until it is finished or skipped, then `child`.
///
/// Inputs: `store` remembers that the flow was seen; `microphonePermission`, when given, adds the
/// microphone screen (only Android asks at run time); `languageController`, when given, adds the
/// transcription screen; `modelSettingsBuilder`, when given, offers to choose another model.
/// Side effects: Reads the store at start; marks the flow seen when it ends; may show the system
/// permission dialog and store the default transcription language.
final class FirstRunFlow extends StatefulWidget {
  /// Creates the flow around `child`.
  const FirstRunFlow({
    required this.store,
    required this.child,
    this.microphonePermission,
    this.languageController,
    this.modelSettingsBuilder,
    super.key,
  });

  final SeenTipsStore store;
  final MicrophonePermission? microphonePermission;
  final TranscriptionLanguageController? languageController;
  final WidgetBuilder? modelSettingsBuilder;

  /// The app, shown once the flow is done.
  final Widget child;

  @override
  State<FirstRunFlow> createState() => _FirstRunFlowState();
}

/// Where the flow stands.
enum _FirstRunState { checking, showing, done }

final class _FirstRunFlowState extends State<FirstRunFlow> {
  _FirstRunState _state = _FirstRunState.checking;
  final PageController _pages = PageController();
  int _page = 0;

  /// Whether the microphone has been allowed from the flow (null: not asked yet).
  bool? _microphoneAllowed;

  @override
  void initState() {
    super.initState();
    unawaited(_check());
  }

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  /// Reads whether the flow was seen; a store that cannot be read counts as seen, so a broken
  /// file never blocks the app.
  Future<void> _check() async {
    bool seen;
    try {
      seen = await widget.store.seen(firstRunTipName);
    } on Object {
      seen = true;
    }
    if (!mounted) {
      return;
    }
    setState(
        () => _state = seen ? _FirstRunState.done : _FirstRunState.showing);
    if (!seen) {
      // The core is open now; the language read at launch may have come too early.
      unawaited(widget.languageController?.load());
    }
  }

  /// Ends the flow for good and shows the app.
  Future<void> _finish() async {
    setState(() => _state = _FirstRunState.done);
    try {
      await widget.store.markSeen(firstRunTipName);
    } on Object {
      // Shown again at the next start; nothing else is lost.
    }
  }

  /// Asks the system for the microphone (and notifications on Android).
  Future<void> _askMicrophone(MicrophonePermission permission) async {
    bool allowed;
    try {
      allowed = await permission.ensureGranted();
    } on Object {
      allowed = false;
    }
    if (mounted) {
      setState(() => _microphoneAllowed = allowed);
    }
  }

  List<Widget> _screens(BuildContext context) => <Widget>[
        _welcome(context),
        if (widget.microphonePermission
            case final MicrophonePermission permission)
          _microphone(context, permission),
        if (widget.languageController
            case final TranscriptionLanguageController language)
          _transcription(context, language),
      ];

  @override
  Widget build(BuildContext context) {
    switch (_state) {
      case _FirstRunState.checking:
        return const Scaffold(body: SizedBox.shrink());
      case _FirstRunState.done:
        return widget.child;
      case _FirstRunState.showing:
        break;
    }
    final List<Widget> screens = _screens(context);
    final bool last = _page >= screens.length - 1;
    final ThemeData theme = Theme.of(context);
    return Scaffold(
      body: SafeArea(
        child: Column(
          children: <Widget>[
            Align(
              alignment: AlignmentDirectional.centerEnd,
              child: Padding(
                padding: const EdgeInsets.all(8),
                child: TextButton(
                  key: firstRunSkipKey,
                  onPressed: _finish,
                  child: Text(l10n.skip),
                ),
              ),
            ),
            Expanded(
              child: PageView(
                controller: _pages,
                onPageChanged: (int page) => setState(() => _page = page),
                children: <Widget>[
                  for (final Widget screen in screens)
                    Center(
                      child: SingleChildScrollView(
                        padding: const EdgeInsets.symmetric(horizontal: 24),
                        child: ConstrainedBox(
                          constraints: const BoxConstraints(maxWidth: 520),
                          child: screen,
                        ),
                      ),
                    ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 24),
              child: Row(
                children: <Widget>[
                  Expanded(
                    child: Text(
                      l10n.firstRunStep(_page + 1, screens.length),
                      style: theme.textTheme.bodyMedium
                          ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                    ),
                  ),
                  const SizedBox(width: 12),
                  FilledButton(
                    key: firstRunNextKey,
                    onPressed: last
                        ? _finish
                        : () => _pages.nextPage(
                              duration: const Duration(milliseconds: 250),
                              curve: Curves.easeOut,
                            ),
                    child: Text(last ? l10n.startUsingKoalino : l10n.next),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// What Koalino does, and the privacy promise.
  Widget _welcome(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        const KoalinoLogo(key: ValueKey<String>('first-run-logo'), size: 96),
        const SizedBox(height: 24),
        Semantics(
          header: true,
          child: Text(l10n.welcomeToKoalino,
              textAlign: TextAlign.center,
              style: theme.textTheme.headlineMedium
                  ?.copyWith(fontWeight: FontWeight.bold)),
        ),
        const SizedBox(height: 12),
        Text(l10n.koalinoIntro,
            textAlign: TextAlign.center, style: theme.textTheme.bodyLarge),
        const SizedBox(height: 24),
        _Promise(icon: Icons.lock_outline, text: l10n.privacyPromise),
      ],
    );
  }

  /// The microphone, asked with its reason.
  Widget _microphone(BuildContext context, MicrophonePermission permission) {
    final ThemeData theme = Theme.of(context);
    final bool? allowed = _microphoneAllowed;
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Icon(Icons.mic_none, size: 72, color: theme.colorScheme.primary),
        const SizedBox(height: 24),
        Semantics(
          header: true,
          child: Text(l10n.microphone,
              textAlign: TextAlign.center,
              style: theme.textTheme.headlineMedium
                  ?.copyWith(fontWeight: FontWeight.bold)),
        ),
        const SizedBox(height: 12),
        Text(l10n.microphoneReason,
            textAlign: TextAlign.center, style: theme.textTheme.bodyLarge),
        const SizedBox(height: 8),
        Text(l10n.notificationsReason,
            textAlign: TextAlign.center,
            style: theme.textTheme.bodyMedium
                ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
        const SizedBox(height: 24),
        if (allowed == true)
          _Promise(
              icon: Icons.check_circle_outline, text: l10n.microphoneAllowed)
        else ...<Widget>[
          FilledButton.tonalIcon(
            key: const ValueKey<String>('first-run-allow-microphone'),
            onPressed: () => _askMicrophone(permission),
            icon: const Icon(Icons.mic),
            label: Text(l10n.allowMicrophone),
          ),
          if (allowed == false) ...<Widget>[
            const SizedBox(height: 12),
            Text(l10n.microphoneNotAllowedYet,
                textAlign: TextAlign.center,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
          ],
        ],
      ],
    );
  }

  /// The transcription language, and the recommended model.
  Widget _transcription(
      BuildContext context, TranscriptionLanguageController language) {
    final ThemeData theme = Theme.of(context);
    return ListenableBuilder(
      listenable: language,
      builder: (BuildContext context, Widget? child) => Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Icon(Icons.translate, size: 72, color: theme.colorScheme.primary),
          const SizedBox(height: 24),
          Semantics(
            header: true,
            child: Text(l10n.transcription,
                textAlign: TextAlign.center,
                style: theme.textTheme.headlineMedium
                    ?.copyWith(fontWeight: FontWeight.bold)),
          ),
          const SizedBox(height: 12),
          Text(l10n.transcriptionLanguageReason,
              textAlign: TextAlign.center, style: theme.textTheme.bodyLarge),
          const SizedBox(height: 16),
          DropdownMenu<String>(
            key: const ValueKey<String>('first-run-language'),
            label: Text(l10n.defaultLanguage),
            initialSelection: language.defaultLanguage,
            enabled: !language.isSaving,
            enableFilter: true,
            requestFocusOnTap: true,
            menuHeight: 320,
            dropdownMenuEntries: <DropdownMenuEntry<String>>[
              for (final LanguageChoice choice in language.choices)
                DropdownMenuEntry<String>(
                    value: choice.code, label: choice.label),
            ],
            onSelected: (String? code) {
              if (code != null) {
                unawaited(language.chooseDefault(code));
              }
            },
          ),
          const SizedBox(height: 24),
          _Promise(icon: Icons.memory, text: l10n.modelRecommended),
          if (widget.modelSettingsBuilder case final WidgetBuilder builder)
            TextButton(
              onPressed: () => Navigator.of(context)
                  .push(MaterialPageRoute<void>(builder: builder)),
              child: Text(l10n.chooseAnotherModel),
            ),
        ],
      ),
    );
  }
}

/// A line with an icon in a soft box: the privacy promise, the recommended model.
final class _Promise extends StatelessWidget {
  const _Promise({required this.icon, required this.text});

  final IconData icon;
  final String text;

  @override
  Widget build(BuildContext context) {
    final ColorScheme colors = Theme.of(context).colorScheme;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: colors.secondaryContainer,
        borderRadius: BorderRadius.circular(16),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Icon(icon, color: colors.onSecondaryContainer),
            const SizedBox(width: 12),
            Expanded(
              child: Text(text,
                  style: TextStyle(color: colors.onSecondaryContainer)),
            ),
          ],
        ),
      ),
    );
  }
}
