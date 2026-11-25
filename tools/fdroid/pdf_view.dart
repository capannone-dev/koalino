// The F-Droid build's PDF view (#163): `tools/fdroid/prepare.sh` copies this file over
// `ui/lib/presentation/pdf_view.dart`. F-Droid builds everything from source, and PDFium's
// library comes prebuilt, so this build shows PDFs like Office documents: a message saying the
// app cannot show them yet. They can still be shared or saved from the project.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// The PDF view without PDFium.
///
/// Inputs: The path of the PDF's clear copy (unused). Outputs: A centred message.
/// Side effects: None.
Widget defaultPdfView(String filePath) => Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            const Icon(Icons.picture_as_pdf_outlined, size: 48),
            const SizedBox(height: 16),
            Text(l10n.thisKindOfDocumentCanT, textAlign: TextAlign.center),
          ],
        ),
      ),
    );
