// Defines the app's Material 3 light and dark themes from one seed colour.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';

/// The brand colour both schemes are generated from: the logo's purple, as in
/// `branding/tokens.json` (#206), which the web dashboard and the browser extension also follow.
const Color noteTakerSeedColor = Color(0xFF571775);

/// Builds the app theme for `brightness`.
///
/// Inputs: The brightness the system asks for. Outputs: A Material 3 theme whose colour scheme
/// is generated from `noteTakerSeedColor`, so light and dark use matching tones and contrast.
/// Side effects: None.
ThemeData noteTakerTheme(Brightness brightness) => ThemeData(
      colorScheme: ColorScheme.fromSeed(
          seedColor: noteTakerSeedColor, brightness: brightness),
      useMaterial3: true,
    );
