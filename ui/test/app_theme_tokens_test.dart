// Checks that the app's colour scheme grows from Koalino's shared brand colour (#206): the seed is
// the logo's purple from branding/tokens.json, which the web dashboard and extension also follow.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/app_theme.dart';

void main() {
  test('the seed colour is the brand colour of branding/tokens.json', () {
    final Map<String, Object?> tokens =
        jsonDecode(File('../branding/tokens.json').readAsStringSync())
            as Map<String, Object?>;
    final String brand = (tokens['brand']! as String).substring(1);
    expect(noteTakerSeedColor, Color(int.parse('ff$brand', radix: 16)));
  });

  test('both themes come from the seed', () {
    for (final Brightness brightness in Brightness.values) {
      expect(
          noteTakerTheme(brightness).colorScheme,
          ColorScheme.fromSeed(
              seedColor: noteTakerSeedColor, brightness: brightness));
    }
  });
}
