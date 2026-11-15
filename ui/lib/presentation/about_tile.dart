// Settings -> About Koalino (#115): the app's name and version, the GPL notice that a
// GPL-3.0-or-later program shows its users, and Flutter's page of the licenses of every package
// the app is built on. The Rust crates and AI models are listed in THIRD_PARTY_NOTICES.md.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// The app's visible name.
const String appName = 'Koalino';

/// The version shown in About; keep in step with `pubspec.yaml`.
const String appVersion = '0.1.0';

/// The license notice, as the GPL asks interactive programs to show it. It stays in English, the
/// language of the license itself (#124).
const String licenseNotice =
    'Koalino is free software: you can redistribute it and/or modify it under the terms of '
    'the GNU General Public License as published by the Free Software Foundation, either '
    'version 3 of the License, or (at your option) any later version.\n\n'
    'Koalino is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; '
    'without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR '
    'PURPOSE. See the GNU General Public License for more details: '
    'https://www.gnu.org/licenses/gpl-3.0.html';

/// A Settings row that opens the About dialog.
final class AboutKoalinoTile extends StatelessWidget {
  /// Creates the row.
  const AboutKoalinoTile({super.key});

  @override
  Widget build(BuildContext context) => ListTile(
        leading: const Icon(Icons.info_outline),
        title: Text(l10n.about(appName)),
        subtitle: Text(l10n.versionLicenseAndTheSoftwareIt),
        onTap: () => showAboutDialog(
          context: context,
          applicationName: appName,
          applicationVersion: appVersion,
          applicationLegalese: licenseNotice,
        ),
      );
}
