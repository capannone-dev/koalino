// The PDF view of the resource viewer (#85): every page, zoomable, drawn by PDFium (`pdfrx`).
//
// It lives in its own file so the F-Droid build (#163) can replace it: `pdfrx` downloads a prebuilt
// PDFium library at build time, which F-Droid does not accept, so `tools/fdroid/prepare.sh` swaps
// in `tools/fdroid/pdf_view.dart` and removes `pdfrx` from the dependencies.

import 'package:flutter/widgets.dart';
import 'package:pdfrx/pdfrx.dart';

/// The default PDF view.
///
/// Inputs: The path of the PDF's clear copy. Outputs: A widget showing every page, zoomable.
/// Side effects: PDFium reads the file while the widget is shown.
Widget defaultPdfView(String filePath) => PdfViewer.file(filePath);
