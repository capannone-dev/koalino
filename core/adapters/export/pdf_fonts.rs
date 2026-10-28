//! Selects, loads, and measures the Unicode fonts the PDF exporter embeds (Issue #37).
//!
//! Text is drawn with the bundled DejaVu Sans faces, which cover Latin, Greek, Cyrillic, and many
//! symbols. Characters DejaVu lacks (CJK, emoji, …) are drawn with the first fallback font that
//! covers them, so one line may switch fonts several times. Fallback fonts are TrueType `.ttf`
//! files found, in order, in `NOTE_TAKER_PDF_FONT_DIR`, `~/.note_taker/fonts/`, and a short list
//! of well-known system font files; they are read from disk only when a document contains a
//! character DejaVu cannot draw. A character no font covers is drawn as U+FFFD (�). If even the
//! bundled font cannot be parsed, rendering falls back to the built-in Helvetica faces with the
//! historical `?` substitution, so a font problem never fails an export.

use std::{
    borrow::Cow,
    cell::{Cell, OnceCell, RefCell},
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

use owned_ttf_parser::{AsFaceRef as _, GlyphId, OwnedFace};
use printpdf::lopdf;

use super::pdf_exporter::FontWeight;

/// DejaVu Sans Book 2.37 (Bitstream Vera license, see `fonts/LICENSE-DejaVu.txt`).
static BUNDLED_REGULAR_FONT: &[u8] = include_bytes!("fonts/DejaVuSans.ttf");
/// DejaVu Sans Bold 2.37.
static BUNDLED_BOLD_FONT: &[u8] = include_bytes!("fonts/DejaVuSans-Bold.ttf");

/// Environment variable naming an extra directory of fallback `.ttf` fonts (highest priority).
const FONT_DIRECTORY_VARIABLE: &str = "NOTE_TAKER_PDF_FONT_DIR";
/// Per-user fallback font directory, relative to the home directory.
const USER_FONT_DIRECTORY: [&str; 2] = [".note_taker", "fonts"];
/// Font files larger than this are skipped rather than read into memory.
const MAX_FONT_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Drawn in place of characters that no available font covers.
const REPLACEMENT_CHARACTER: char = '\u{FFFD}';
/// Conservative average Helvetica glyph width as a fraction of the font size; the built-in fonts
/// expose no metrics here, so their text is measured with this estimate.
const BUILTIN_AVERAGE_GLYPH_WIDTH_EM: f32 = 0.52;

/// Well-known TrueType system fonts that add CJK or monochrome emoji coverage. Missing files are
/// skipped silently. TrueType collections (`.ttc`) and CFF-based `.otf` fonts are not listed:
/// printpdf can only embed single TrueType-outline fonts.
const SYSTEM_FALLBACK_FONT_FILES: &[&str] = &[
    // Linux (Debian/Ubuntu, Fedora, Arch package layouts).
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/google-droid-sans-fonts/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/droid/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/TTF/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/truetype/ancient-scripts/Symbola_hint.ttf",
    "/usr/share/fonts/TTF/Symbola.ttf",
    // macOS.
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
    "/System/Library/Fonts/Apple Symbols.ttf",
];

/// Windows font files (inside `%WINDIR%\Fonts`) that add CJK or monochrome emoji coverage.
const WINDOWS_FALLBACK_FONT_FILES: &[&str] = &["simhei.ttf", "malgun.ttf", "seguisym.ttf"];

/// Where the PDF exporter finds its fonts.
///
/// The primary fonts are the bundled DejaVu Sans faces. Fallback fonts are discovered lazily from
/// the directories (every `.ttf` file, sorted by name) and then the individual files listed here.
#[derive(Clone)]
pub struct PdfFontConfig {
    /// Whether the primary fonts are the bundled DejaVu faces, which share no glyphs between code
    /// points (verified by a unit test), so their `cmap` need not be scanned on every export.
    primary_fonts_are_bundled: bool,
    regular_font_bytes: Cow<'static, [u8]>,
    bold_font_bytes: Cow<'static, [u8]>,
    fallback_directories: Vec<PathBuf>,
    fallback_files: Vec<PathBuf>,
}

impl PdfFontConfig {
    /// Returns a configuration with the bundled fonts and no fallback fonts at all.
    ///
    /// Inputs: None.
    /// Outputs: A configuration that never reads font files from disk.
    /// Side effects: None.
    pub fn bundled() -> Self {
        Self {
            primary_fonts_are_bundled: true,
            regular_font_bytes: Cow::Borrowed(BUNDLED_REGULAR_FONT),
            bold_font_bytes: Cow::Borrowed(BUNDLED_BOLD_FONT),
            fallback_directories: Vec::new(),
            fallback_files: Vec::new(),
        }
    }

    /// Reads the fallback font locations through `lookup`, which returns an environment
    /// variable's value.
    ///
    /// Inputs: A variable lookup; blank values count as unset. Directories are
    /// `NOTE_TAKER_PDF_FONT_DIR`, then `$HOME/.note_taker/fonts` (or `%USERPROFILE%`); files are
    /// the well-known system fonts, plus `%WINDIR%\Fonts` ones when `WINDIR` is set.
    /// Outputs: The configuration; nothing is checked for existence yet.
    /// Side effects: None beyond calling `lookup`.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let non_blank = |value: &String| !value.trim().is_empty();
        let configured_directory = lookup(FONT_DIRECTORY_VARIABLE)
            .filter(non_blank)
            .map(PathBuf::from);
        let user_directory = lookup("HOME")
            .filter(non_blank)
            .or_else(|| lookup("USERPROFILE").filter(non_blank))
            .map(|home_directory| {
                USER_FONT_DIRECTORY
                    .iter()
                    .fold(PathBuf::from(home_directory), |path, segment| {
                        path.join(segment)
                    })
            });
        let windows_font_directory = lookup("WINDIR")
            .filter(non_blank)
            .map(|windows_directory| PathBuf::from(windows_directory).join("Fonts"));
        let mut fallback_files = SYSTEM_FALLBACK_FONT_FILES
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if let Some(windows_font_directory) = windows_font_directory {
            fallback_files.extend(
                WINDOWS_FALLBACK_FONT_FILES
                    .iter()
                    .map(|file_name| windows_font_directory.join(file_name)),
            );
        }
        Self {
            fallback_directories: configured_directory
                .into_iter()
                .chain(user_directory)
                .collect(),
            fallback_files,
            ..Self::bundled()
        }
    }

    /// Reads the fallback font locations from the process environment.
    ///
    /// Inputs: None.
    /// Outputs: See `from_lookup`.
    /// Side effects: Reads environment variables.
    pub fn from_environment() -> Self {
        Self::from_lookup(|variable_name| std::env::var(variable_name).ok())
    }

    /// Replaces every fallback location with `fallback_directories` (and no individual files).
    ///
    /// Inputs: Directories whose `.ttf` files become fallback fonts, in priority order.
    /// Outputs: The updated configuration.
    /// Side effects: None.
    pub fn with_fallback_directories(mut self, fallback_directories: Vec<PathBuf>) -> Self {
        self.fallback_directories = fallback_directories;
        self.fallback_files = Vec::new();
        self
    }

    /// Replaces the bundled primary fonts, so tests can simulate an unusable bundled font.
    #[cfg(test)]
    pub(crate) fn with_primary_fonts(mut self, regular: Vec<u8>, bold: Vec<u8>) -> Self {
        self.primary_fonts_are_bundled = false;
        self.regular_font_bytes = Cow::Owned(regular);
        self.bold_font_bytes = Cow::Owned(bold);
        self
    }

    /// Directories scanned for fallback `.ttf` fonts, in priority order.
    pub fn fallback_directories(&self) -> &[PathBuf] {
        &self.fallback_directories
    }

    /// Individual fallback font files tried after the directories.
    pub fn fallback_files(&self) -> &[PathBuf] {
        &self.fallback_files
    }
}

/// A parsed TrueType font: its bytes (embedded on use) and its metrics.
pub(crate) struct LoadedFont {
    face: OwnedFace,
    /// Per glyph ID, whether several code points map to it; computed on first use.
    shared_glyphs: OnceCell<Vec<bool>>,
}

impl LoadedFont {
    /// Parses `font_bytes`, accepting only single TrueType-outline fonts.
    ///
    /// Inputs: Raw font file bytes.
    /// Outputs: The font, or a short reason it cannot be embedded (for logs).
    /// Side effects: None.
    fn parse(font_bytes: Vec<u8>) -> Result<Self, &'static str> {
        if owned_ttf_parser::fonts_in_collection(&font_bytes).is_some() {
            return Err("font collections (.ttc) cannot be embedded");
        }
        let face = OwnedFace::from_vec(font_bytes, 0).map_err(|_| "not a parsable font")?;
        if face.as_face_ref().tables().glyf.is_none() {
            return Err("only TrueType-outline fonts can be embedded");
        }
        Ok(Self {
            face,
            shared_glyphs: OnceCell::new(),
        })
    }

    /// Records that no glyph is shared, skipping the `cmap` scan (only for fonts known to qualify).
    fn with_no_shared_glyphs(self) -> Self {
        let _ = self.shared_glyphs.set(Vec::new());
        self
    }

    /// Reports whether any glyph is shared by several code points (scans the `cmap` once).
    #[cfg(test)]
    pub(crate) fn has_shared_glyphs(&self) -> bool {
        let glyph_count = self.face.as_face_ref().number_of_glyphs();
        (0..glyph_count).any(|glyph_index| self.is_shared_glyph(GlyphId(glyph_index)))
    }

    /// Returns the raw font bytes, which printpdf subsets and embeds.
    pub(crate) fn font_bytes(&self) -> &[u8] {
        self.face.as_slice()
    }

    /// Returns the glyph for `character`, if the font has one.
    fn glyph(&self, character: char) -> Option<GlyphId> {
        self.face
            .as_face_ref()
            .glyph_index(character)
            .filter(|glyph_id| glyph_id.0 != 0)
    }

    /// Reports whether more than one code point maps to `glyph_id`, which makes printpdf's
    /// `ToUnicode` entry for it ambiguous.
    fn is_shared_glyph(&self, glyph_id: GlyphId) -> bool {
        self.shared_glyphs
            .get_or_init(|| {
                let face = self.face.as_face_ref();
                let glyph_count = usize::from(face.number_of_glyphs());
                let mut is_shared = vec![false; glyph_count];
                let unicode_subtables = face
                    .tables()
                    .cmap
                    .into_iter()
                    .flat_map(|cmap| cmap.subtables.into_iter())
                    .filter(|subtable| subtable.is_unicode());
                // Per subtable: format 4 and format 12 subtables repeat the same mappings.
                for unicode_subtable in unicode_subtables {
                    let mut is_mapped = vec![false; glyph_count];
                    unicode_subtable.codepoints(|code_point| {
                        let Some(mapped_glyph) = unicode_subtable.glyph_index(code_point) else {
                            return;
                        };
                        let glyph_index = usize::from(mapped_glyph.0);
                        if let Some(was_mapped) = is_mapped.get_mut(glyph_index) {
                            if *was_mapped && let Some(shared) = is_shared.get_mut(glyph_index) {
                                *shared = true;
                            }
                            *was_mapped = true;
                        }
                    });
                }
                is_shared
            })
            .get(usize::from(glyph_id.0))
            .is_some_and(|shared| *shared)
    }

    /// Returns the horizontal advance of `glyph_id` as a fraction of the font size.
    fn advance_em(&self, glyph_id: GlyphId) -> f32 {
        let face = self.face.as_face_ref();
        let units_per_em = f32::from(face.units_per_em().max(1));
        face.glyph_hor_advance(glyph_id)
            .map_or(0.0, |advance| f32::from(advance) / units_per_em)
    }
}

/// How one source character is drawn.
#[derive(Clone, Copy)]
struct ResolvedCharacter {
    font_slot: FontSlot,
    /// The character actually drawn: U+FFFD for uncovered ones, `?` with the built-in fonts.
    drawn_character: char,
    /// The glyph in the slot's font (unused with the built-in fonts).
    glyph_id: u16,
    /// Horizontal advance as a fraction of the font size.
    advance_em: f32,
}

/// A memoized `resolve` result: `None` until resolved; `Some(None)` for dropped characters.
type CachedResolution = Cell<Option<Option<ResolvedCharacter>>>;

/// The font a run of text is drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FontSlot {
    /// One of the two primary faces (bundled DejaVu Sans, or built-in Helvetica).
    Primary(FontWeight),
    /// The fallback font at this index of `EmbeddedFonts::fallback_fonts`.
    Fallback(usize),
}

/// A maximal sequence of characters drawn with one font.
pub(crate) struct TextRun {
    pub(crate) font_slot: FontSlot,
    pub(crate) text: String,
    /// The glyph of each character of `text` in the slot's font; empty with the built-in fonts.
    pub(crate) glyph_ids: Vec<u16>,
    /// Total advance width as a fraction of the font size.
    pub(crate) width_em: f32,
}

/// Fonts loaded for one export, with lazily discovered fallbacks and usage tracking.
pub(crate) struct EmbeddedFonts {
    regular: LoadedFont,
    bold: LoadedFont,
    fallback_directories: Vec<PathBuf>,
    fallback_files: Vec<PathBuf>,
    /// Fallback fonts, loaded on the first character the primary fonts cannot draw.
    fallback_fonts: OnceCell<Vec<LoadedFont>>,
    /// Indices of fallback fonts that drew at least one character (only these are embedded).
    used_fallback_indices: RefCell<BTreeSet<usize>>,
    /// Memoized `resolve` results for ASCII, indexed by `ascii_cache_index`; an array because
    /// transcripts are mostly ASCII and layout and drawing resolve every character several times.
    resolved_ascii_characters: [CachedResolution; 2 * ASCII_CHARACTER_COUNT],
    /// Memoized `resolve` results for every other character.
    resolved_characters: RefCell<HashMap<(char, FontWeight), Option<ResolvedCharacter>>>,
    /// Distinct characters drawn as U+FFFD because no font covers them.
    uncovered_character_count: Cell<usize>,
    /// Whether printpdf's `ToUnicode` maps would be wrong for a drawn character: one above U+FFFF,
    /// or one whose glyph other code points share.
    needs_unicode_map_rebuild: Cell<bool>,
}

/// The fonts text is measured and drawn with.
pub(crate) enum TextFonts {
    /// Embedded Unicode TrueType fonts (the normal case).
    Embedded(Box<EmbeddedFonts>),
    /// Built-in Helvetica faces (WinAnsi only), used only when the bundled font is unusable.
    Builtin,
}

impl TextFonts {
    /// Parses the configured primary fonts; fallback fonts are not touched yet.
    ///
    /// Inputs: The font configuration.
    /// Outputs: Embedded fonts, or `Builtin` when a primary font cannot be parsed.
    /// Side effects: Logs a warning when falling back to the built-in fonts.
    pub(crate) fn load(font_config: &PdfFontConfig) -> Self {
        let primary_fonts =
            LoadedFont::parse(font_config.regular_font_bytes.to_vec()).and_then(|regular| {
                LoadedFont::parse(font_config.bold_font_bytes.to_vec()).map(|bold| (regular, bold))
            });
        match primary_fonts {
            Ok((regular, bold)) if font_config.primary_fonts_are_bundled => {
                Self::from_primary_fonts(
                    regular.with_no_shared_glyphs(),
                    bold.with_no_shared_glyphs(),
                    font_config,
                )
            }
            Ok((regular, bold)) => Self::from_primary_fonts(regular, bold, font_config),
            Err(reason) => {
                tracing::warn!(
                    reason,
                    "PDF export could not load its bundled font; using built-in Helvetica"
                );
                Self::Builtin
            }
        }
    }

    /// Wraps parsed primary fonts with the configured fallback locations.
    fn from_primary_fonts(
        regular: LoadedFont,
        bold: LoadedFont,
        font_config: &PdfFontConfig,
    ) -> Self {
        Self::Embedded(Box::new(EmbeddedFonts {
            regular,
            bold,
            fallback_directories: font_config.fallback_directories.clone(),
            fallback_files: font_config.fallback_files.clone(),
            fallback_fonts: OnceCell::new(),
            used_fallback_indices: RefCell::new(BTreeSet::new()),
            resolved_ascii_characters: std::array::from_fn(|_| Cell::new(None)),
            resolved_characters: RefCell::new(HashMap::new()),
            uncovered_character_count: Cell::new(0),
            needs_unicode_map_rebuild: Cell::new(false),
        }))
    }

    /// Splits `text` into runs that each use one font, substituting what no font can draw.
    ///
    /// Inputs: One line of text and the requested weight.
    /// Outputs: Runs in reading order; tabs become spaces, invisible format characters no font
    /// covers are dropped, and other uncovered characters become U+FFFD (or `?` with the built-in
    /// fonts).
    /// Side effects: May load fallback fonts from disk (once); records which fallbacks are used.
    pub(crate) fn runs(&self, text: &str, weight: FontWeight) -> Vec<TextRun> {
        let mut runs: Vec<TextRun> = Vec::new();
        for source_character in text.chars() {
            let Some(resolved_character) = self.resolve(source_character, weight) else {
                continue;
            };
            let continues_last_run = runs
                .last()
                .is_some_and(|last_run| last_run.font_slot == resolved_character.font_slot);
            if !continues_last_run {
                runs.push(TextRun {
                    font_slot: resolved_character.font_slot,
                    text: String::new(),
                    glyph_ids: Vec::new(),
                    width_em: 0.0,
                });
            }
            if let Some(current_run) = runs.last_mut() {
                current_run.text.push(resolved_character.drawn_character);
                current_run.width_em += resolved_character.advance_em;
                if matches!(self, Self::Embedded(_)) {
                    current_run.glyph_ids.push(resolved_character.glyph_id);
                }
            }
        }
        runs
    }

    /// Measures `text` as it will be drawn.
    ///
    /// Inputs: One line of text, a font size in points, and the weight.
    /// Outputs: The width in millimetres.
    /// Side effects: As `runs`.
    pub(crate) fn text_width_mm(&self, text: &str, font_size: f32, weight: FontWeight) -> f32 {
        let width_em: f32 = text
            .chars()
            .filter_map(|character| self.resolve(character, weight))
            .map(|resolved_character| resolved_character.advance_em)
            .sum();
        width_em * font_size * super::pdf_exporter::MM_PER_POINT
    }

    /// Returns the embedded fonts, or `None` in built-in mode.
    pub(crate) fn embedded(&self) -> Option<&EmbeddedFonts> {
        match self {
            Self::Embedded(embedded_fonts) => Some(embedded_fonts),
            Self::Builtin => None,
        }
    }

    /// Picks the font, the character actually drawn, and its advance for `source_character`.
    fn resolve(&self, source_character: char, weight: FontWeight) -> Option<ResolvedCharacter> {
        let character = if source_character == '\t' {
            ' '
        } else {
            source_character
        };
        match self {
            Self::Embedded(embedded_fonts) => embedded_fonts.resolve(character, weight),
            Self::Builtin => {
                let is_encodable = !character.is_control()
                    && !lopdf::Document::encode_text(
                        Some("WinAnsiEncoding"),
                        &character.to_string(),
                    )
                    .is_empty();
                Some(ResolvedCharacter {
                    font_slot: FontSlot::Primary(weight),
                    drawn_character: if is_encodable { character } else { '?' },
                    glyph_id: 0,
                    advance_em: BUILTIN_AVERAGE_GLYPH_WIDTH_EM,
                })
            }
        }
    }
}

impl EmbeddedFonts {
    /// Returns the primary face for `weight`.
    pub(crate) fn primary(&self, weight: FontWeight) -> &LoadedFont {
        match weight {
            FontWeight::Regular => &self.regular,
            FontWeight::Bold => &self.bold,
        }
    }

    /// Returns the fallback fonts that drew at least one character, with their indices.
    pub(crate) fn used_fallback_fonts(&self) -> Vec<(usize, &LoadedFont)> {
        let used_fallback_indices = self.used_fallback_indices.borrow();
        self.fallback_fonts
            .get()
            .map(|fallback_fonts| {
                fallback_fonts
                    .iter()
                    .enumerate()
                    .filter(|(fallback_index, _)| used_fallback_indices.contains(fallback_index))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Number of distinct characters drawn as U+FFFD so far.
    pub(crate) fn uncovered_character_count(&self) -> usize {
        self.uncovered_character_count.get()
    }

    /// Whether `ToUnicode` maps must be rebuilt (see `unicode_cmap`) for text to copy back exactly.
    pub(crate) fn needs_unicode_map_rebuild(&self) -> bool {
        self.needs_unicode_map_rebuild.get()
    }

    /// Every character resolved so far (layout measures all drawn text), as drawn.
    pub(crate) fn drawn_characters(&self) -> BTreeSet<char> {
        let resolved_characters = self.resolved_characters.borrow();
        self.resolved_ascii_characters
            .iter()
            .filter_map(Cell::get)
            .chain(resolved_characters.values().copied())
            .flatten()
            .map(|resolved_character| resolved_character.drawn_character)
            .collect()
    }

    /// Picks the first font covering `character` (see `resolve_uncached`), memoized per weight.
    fn resolve(&self, character: char, weight: FontWeight) -> Option<ResolvedCharacter> {
        if let Some(cached_ascii) = ascii_cache_index(character, weight)
            .and_then(|cache_index| self.resolved_ascii_characters.get(cache_index))
        {
            return cached_ascii.get().unwrap_or_else(|| {
                let resolved_character = self.resolve_uncached(character, weight);
                cached_ascii.set(Some(resolved_character));
                resolved_character
            });
        }
        if let Some(resolved_character) =
            self.resolved_characters.borrow().get(&(character, weight))
        {
            return *resolved_character;
        }
        let resolved_character = self.resolve_uncached(character, weight);
        self.resolved_characters
            .borrow_mut()
            .insert((character, weight), resolved_character);
        resolved_character
    }

    /// Picks the first font covering `character`: the requested primary face, the other primary
    /// face, then each fallback in priority order. Records fallback usage and coverage gaps.
    fn resolve_uncached(&self, character: char, weight: FontWeight) -> Option<ResolvedCharacter> {
        let other_weight = match weight {
            FontWeight::Regular => FontWeight::Bold,
            FontWeight::Bold => FontWeight::Regular,
        };
        let covering_font = (!character.is_control())
            .then(|| {
                [weight, other_weight]
                    .into_iter()
                    .find_map(|primary_weight| {
                        let primary_font = self.primary(primary_weight);
                        primary_font.glyph(character).map(|glyph_id| {
                            (FontSlot::Primary(primary_weight), primary_font, glyph_id)
                        })
                    })
                    .or_else(|| {
                        self.fallback_fonts().iter().enumerate().find_map(
                            |(fallback_index, fallback_font)| {
                                fallback_font.glyph(character).map(|glyph_id| {
                                    (FontSlot::Fallback(fallback_index), fallback_font, glyph_id)
                                })
                            },
                        )
                    })
            })
            .flatten();
        match covering_font {
            Some((font_slot, font, glyph_id)) => {
                if u32::from(character) > 0xFFFF || font.is_shared_glyph(glyph_id) {
                    self.needs_unicode_map_rebuild.set(true);
                }
                if let FontSlot::Fallback(fallback_index) = font_slot {
                    self.used_fallback_indices
                        .borrow_mut()
                        .insert(fallback_index);
                }
                Some(ResolvedCharacter {
                    font_slot,
                    drawn_character: character,
                    glyph_id: glyph_id.0,
                    advance_em: font.advance_em(glyph_id),
                })
            }
            None if is_invisible_format_character(character) => None,
            None => {
                self.uncovered_character_count
                    .set(self.uncovered_character_count.get() + 1);
                let primary_font = self.primary(weight);
                // Without a U+FFFD glyph, glyph 0 (`.notdef`, usually a box) is drawn instead.
                let replacement_glyph = primary_font.glyph(REPLACEMENT_CHARACTER);
                Some(ResolvedCharacter {
                    font_slot: FontSlot::Primary(weight),
                    drawn_character: REPLACEMENT_CHARACTER,
                    glyph_id: replacement_glyph.map_or(0, |glyph_id| glyph_id.0),
                    advance_em: replacement_glyph
                        .map_or(0.0, |glyph_id| primary_font.advance_em(glyph_id)),
                })
            }
        }
    }

    /// Loads the fallback fonts on first use.
    fn fallback_fonts(&self) -> &[LoadedFont] {
        self.fallback_fonts
            .get_or_init(|| load_fallback_fonts(&self.fallback_directories, &self.fallback_files))
    }
}

/// Builds a `ToUnicode` CMap for an embedded TrueType program, mapping each glyph ID to the
/// character it was drawn for.
///
/// Inputs: The embedded (subset) font bytes and every character drawn with embedded fonts. When
/// several code points share a glyph, a drawn one wins; otherwise the lowest code point does.
/// Outputs: The CMap source with UTF-16BE destinations (surrogate pairs above U+FFFF), or `None`
/// when the program cannot be parsed or has no Unicode `cmap`.
/// Side effects: None.
pub(crate) fn unicode_cmap(
    font_program: &[u8],
    drawn_characters: &BTreeSet<char>,
) -> Option<String> {
    let face = owned_ttf_parser::Face::parse(font_program, 0).ok()?;
    let mut character_by_glyph = BTreeMap::<u16, char>::new();
    for unicode_subtable in face
        .tables()
        .cmap?
        .subtables
        .into_iter()
        .filter(|subtable| subtable.is_unicode())
    {
        unicode_subtable.codepoints(|code_point| {
            let Some(character) = char::from_u32(code_point) else {
                return;
            };
            let Some(glyph_id) = unicode_subtable
                .glyph_index(code_point)
                .filter(|glyph_id| glyph_id.0 != 0)
            else {
                return;
            };
            let mapped_character = character_by_glyph.entry(glyph_id.0).or_insert(character);
            let keeps_drawn_mapping = drawn_characters.contains(mapped_character);
            if !keeps_drawn_mapping
                && (drawn_characters.contains(&character) || character < *mapped_character)
            {
                *mapped_character = character;
            }
        });
    }
    let mut unicode_cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let mappings = character_by_glyph.into_iter().collect::<Vec<_>>();
    // A bfchar block may hold at most 100 entries.
    for mapping_block in mappings.chunks(100) {
        unicode_cmap.push_str(&format!("{} beginbfchar\n", mapping_block.len()));
        for (glyph_id, character) in mapping_block {
            let utf16_hex = character
                .encode_utf16(&mut [0_u16; 2])
                .iter()
                .map(|code_unit| format!("{code_unit:04X}"))
                .collect::<String>();
            unicode_cmap.push_str(&format!("<{glyph_id:04X}> <{utf16_hex}>\n"));
        }
        unicode_cmap.push_str("endbfchar\n");
    }
    unicode_cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    Some(unicode_cmap)
}

/// Parses the bundled regular and bold fonts, without the shortcuts `TextFonts::load` applies.
#[cfg(test)]
pub(crate) fn parse_bundled_fonts() -> Result<[LoadedFont; 2], &'static str> {
    Ok([
        LoadedFont::parse(BUNDLED_REGULAR_FONT.to_vec())?,
        LoadedFont::parse(BUNDLED_BOLD_FONT.to_vec())?,
    ])
}

/// Number of ASCII characters, the size of each weight's fast resolution cache.
const ASCII_CHARACTER_COUNT: usize = 128;

/// Returns `character`'s slot in `EmbeddedFonts::resolved_ascii_characters`, or `None` for
/// non-ASCII characters.
fn ascii_cache_index(character: char, weight: FontWeight) -> Option<usize> {
    let weight_offset = match weight {
        FontWeight::Regular => 0,
        FontWeight::Bold => ASCII_CHARACTER_COUNT,
    };
    character
        .is_ascii()
        .then(|| weight_offset + usize::from(character as u8))
}

/// Loads every usable `.ttf` font from `directories` (sorted by file name), then `files`.
///
/// Inputs: Fallback directories and files in priority order; missing ones are skipped.
/// Outputs: The fonts that could be parsed.
/// Side effects: Reads font files; logs (by path only) each existing font that is skipped.
fn load_fallback_fonts(directories: &[PathBuf], files: &[PathBuf]) -> Vec<LoadedFont> {
    let directory_font_paths = directories.iter().flat_map(|directory| {
        let mut font_paths = fs::read_dir(directory)
            .map(|directory_entries| {
                directory_entries
                    .filter_map(Result::ok)
                    .map(|directory_entry| directory_entry.path())
                    .filter(|candidate_path| has_ttf_extension(candidate_path))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        font_paths.sort();
        font_paths
    });
    directory_font_paths
        .chain(
            files
                .iter()
                .filter(|file_path| file_path.is_file())
                .cloned(),
        )
        .filter_map(|font_path| load_font_file(&font_path))
        .collect()
}

/// Reads and parses one fallback font file.
///
/// Inputs: A path expected to hold a TrueType font.
/// Outputs: The font, or `None` when it is missing, too large, unreadable, or not embeddable.
/// Side effects: Reads the file; logs a warning (by path) when an existing file is skipped.
fn load_font_file(font_path: &Path) -> Option<LoadedFont> {
    let skip = |reason: &str| {
        tracing::warn!(font_path = %font_path.display(), reason, "PDF export skipped a fallback font");
    };
    let file_size = fs::metadata(font_path).ok()?.len();
    if file_size > MAX_FONT_FILE_BYTES {
        skip("font file is larger than 64 MiB");
        return None;
    }
    let font_bytes = fs::read(font_path)
        .map_err(|_| skip("font file is unreadable"))
        .ok()?;
    LoadedFont::parse(font_bytes).map_err(skip).ok()
}

/// Reports whether `candidate_path` ends in `.ttf` (any case).
fn has_ttf_extension(candidate_path: &Path) -> bool {
    candidate_path.is_file()
        && candidate_path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("ttf"))
}

/// Reports whether `character` is an invisible formatting character (zero-width joiners and
/// spaces, variation selectors, tag characters) that can be dropped when no font covers it.
fn is_invisible_format_character(character: char) -> bool {
    matches!(
        u32::from(character),
        0x00AD | 0x180B..=0x180F | 0x200B..=0x200F | 0x2060..=0x206F | 0xFE00..=0xFE0F | 0xFEFF
            | 0xE0000..=0xE0FFF
    )
}
