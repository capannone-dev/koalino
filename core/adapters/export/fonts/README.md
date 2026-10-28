<!-- This directory holds the Unicode fonts compiled into the PDF exporter. -->

# Bundled PDF fonts

`pdf_fonts.rs` compiles these fonts into the binary with `include_bytes!`. They are the primary
faces of every exported PDF (Issue #37), so Latin, Greek, and Cyrillic text renders without
reading any font from disk. Characters they lack fall back to user or system fonts; see
[PDF fonts](../README.md#pdf-fonts).

| File | Contents |
|---|---|
| `DejaVuSans.ttf` | DejaVu Sans Book 2.37, the regular face |
| `DejaVuSans-Bold.ttf` | DejaVu Sans Bold 2.37, used for headings and speaker labels |
| `LICENSE-DejaVu.txt` | The DejaVu license: Bitstream Vera terms for the base glyphs, public domain for DejaVu's changes, and Arev terms for imported glyphs |

The license allows bundling and redistribution, and requires that this license file ship with
the fonts. Only the glyphs a PDF uses are embedded in it.

Replacing these files changes every export. A replacement must be a single TrueType-outline font,
and the `bundled_fonts_share_no_glyphs_between_code_points` test must still pass (see
`pdf_unicode_tests.rs`).
