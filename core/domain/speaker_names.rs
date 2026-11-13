//! Turns the names a call app shows into name suggestions for unknown voices (#113).
//!
//! Call apps show who is speaking: a tile labelled "Alice is speaking", a "Talking: Bruno" bar,
//! or caption lines with the speaker's name. While a recording runs, the desktop reads the call
//! window's accessibility labels (`speaking_name` picks the name out of one) and collects the
//! readings in a `SpeakingTimeline`, stored with the meeting as WebVTT with one voice-tagged cue
//! per span. Teams captions from the browser extension (#102) use the same format, so
//! `parse_voice_cues` reads both. After the meeting, `suggest_names` compares each unknown voice's
//! turns with those spans: a voice heard mostly while one name was shown is offered that name in
//! the voice gallery. Nothing is named without the owner's tap.
//!
//! The label patterns are English and Italian phrases used by web and desktop call apps. They
//! were written from the apps' visible texts, not from captured accessibility trees, so an app
//! whose labels differ gives no readings until a pattern is added here.

use std::collections::HashMap;

use uuid::Uuid;

/// A reading counts this long before and after the moment it was taken, in milliseconds.
pub const READING_REACH_MS: u64 = 1_000;
/// Readings of one name closer than this, in milliseconds, join into one span.
pub const MERGE_GAP_MS: u64 = 3_000;
/// A voice needs at least this much speech under named spans, in milliseconds, for a suggestion.
pub const MIN_EVIDENCE_MS: u64 = 2_000;
/// The suggested name must cover at least this share of the voice's named speech.
pub const MIN_NAME_SHARE: f64 = 0.6;
/// Longest accepted name, in characters.
const MAX_NAME_CHARS: usize = 60;
/// Most words in an accepted name.
const MAX_NAME_WORDS: usize = 5;

/// Words that say someone speaks, as a label's last comma-separated part or its suffix.
const SPEAKING_WORDS: [&str; 4] = ["speaking", "talking", "sta parlando", "parla"];
/// Label suffixes after the name: "Alice is speaking".
const SPEAKING_SUFFIXES: [&str; 3] = [" is speaking", " is talking", " sta parlando"];
/// Label prefixes before the name: "Talking: Alice".
const SPEAKING_PREFIXES: [&str; 3] = ["talking:", "speaking:", "sta parlando:"];
/// Words that show the text is not a person's name.
const NOT_A_NAME: [&str; 8] = [
    "someone", "anyone", "everyone", "nobody", "you", "qualcuno", "nessuno", "tu",
];
/// Window-title fragments of call apps and meeting sites (lowercase).
const CALL_WINDOW_MARKERS: [&str; 12] = [
    "teams",
    "meet - ",
    "google meet",
    "zoom",
    "jitsi",
    "webex",
    "whereby",
    "discord",
    "huddle",
    "skype",
    "bigbluebutton",
    "gotomeeting",
];

/// A time span during which a name was shown as speaking, in meeting milliseconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedSpan {
    /// Start, in milliseconds from the meeting start.
    pub start_ms: u64,
    /// End, in milliseconds from the meeting start.
    pub end_ms: u64,
    /// The name as shown.
    pub name: String,
}

impl NamedSpan {
    /// Creates a span. Inputs: start and end (meeting milliseconds) and the name.
    /// Outputs: The span. Side effects: None.
    pub fn new(start_ms: u64, end_ms: u64, name: &str) -> Self {
        Self {
            start_ms,
            end_ms,
            name: name.to_owned(),
        }
    }
}

/// One stretch of speech by an unknown voice, in meeting milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceTurn {
    /// The unknown-speaker cluster that spoke.
    pub cluster_id: Uuid,
    /// Start, in milliseconds from the meeting start.
    pub start_ms: u64,
    /// End, in milliseconds from the meeting start.
    pub end_ms: u64,
}

/// Picks the speaker's name out of one accessibility label.
///
/// Inputs: A label or text of a call window's element.
/// Outputs: The name when the label says that person is speaking ("Alice is speaking",
/// "Talking: Alice", "Alice, video is off, speaking", "Alice (speaking)", Italian forms), and
/// the name looks like a person's name (at most 5 words and 60 characters, not "you" or
/// "someone"); otherwise `None`. Side effects: None.
pub fn speaking_name(label: &str) -> Option<String> {
    let trimmed = label.trim();
    let lower = trimmed.to_lowercase();
    // Byte offsets of `lower` match `trimmed` only for ASCII case changes, so cut on `lower`'s
    // lengths only after checking the label is unchanged in length.
    let same_length = lower.len() == trimmed.len();
    if same_length {
        for prefix in SPEAKING_PREFIXES {
            if lower.starts_with(prefix) {
                return plausible_name(&trimmed[prefix.len()..]);
            }
        }
        for suffix in SPEAKING_SUFFIXES {
            if lower.ends_with(suffix) {
                return plausible_name(&trimmed[..trimmed.len() - suffix.len()]);
            }
        }
        if lower.ends_with("(speaking)") {
            return plausible_name(&trimmed[..trimmed.len() - "(speaking)".len()]);
        }
    }
    let mut parts = trimmed.split(',').map(str::trim);
    let first_part = parts.next()?;
    let says_speaking = parts.any(|part| SPEAKING_WORDS.contains(&part.to_lowercase().as_str()));
    if says_speaking {
        return plausible_name(first_part);
    }
    None
}

/// Accepts text as a name when it looks like one.
///
/// Outputs: The text with spaces collapsed, or `None` when empty, too long, of too many words,
/// without a letter, or containing a word such as "you" or "someone". Side effects: None.
fn plausible_name(candidate: &str) -> Option<String> {
    let words: Vec<&str> = candidate.split_whitespace().collect();
    let name = words.join(" ");
    let is_plausible = !words.is_empty()
        && words.len() <= MAX_NAME_WORDS
        && name.chars().count() <= MAX_NAME_CHARS
        && name.chars().any(char::is_alphabetic)
        && !words
            .iter()
            .any(|word| NOT_A_NAME.contains(&word.to_lowercase().as_str()));
    is_plausible.then_some(name)
}

/// Tells whether a window title belongs to a call app or meeting site.
///
/// Inputs: A window or frame title. Outputs: `true` for Teams, Meet, Zoom, Jitsi, Webex,
/// Whereby, Discord, Slack huddles, Skype, BigBlueButton and GoToMeeting. Side effects: None.
pub fn is_call_window(title: &str) -> bool {
    let lower = title.to_lowercase();
    CALL_WINDOW_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

/// Collects the names read during one recording as spans per name.
#[derive(Clone, Debug, Default)]
pub struct SpeakingTimeline {
    /// Finished and open spans, in the order they started.
    spans: Vec<NamedSpan>,
}

impl SpeakingTimeline {
    /// Records the names shown as speaking at one moment.
    ///
    /// Inputs: The meeting time of the reading and the names read (possibly none).
    /// Outputs: None. Side effects: Extends the name's last span when it ended less than
    /// `MERGE_GAP_MS` before, or starts a span of `READING_REACH_MS` either side.
    pub fn observe(&mut self, meeting_time_ms: u64, names: &[String]) {
        let start_ms = meeting_time_ms.saturating_sub(READING_REACH_MS);
        let end_ms = meeting_time_ms.saturating_add(READING_REACH_MS);
        for name in names {
            let open_span = self.spans.iter_mut().rev().find(|span| span.name == *name);
            match open_span {
                Some(span) if start_ms <= span.end_ms.saturating_add(MERGE_GAP_MS) => {
                    span.end_ms = span.end_ms.max(end_ms);
                }
                _ => self.spans.push(NamedSpan::new(start_ms, end_ms, name)),
            }
        }
    }

    /// Reports whether nothing was read.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Returns the spans ordered by start. Side effects: None.
    pub fn spans(&self) -> Vec<NamedSpan> {
        let mut spans = self.spans.clone();
        spans.sort_by_key(|span| (span.start_ms, span.end_ms));
        spans
    }

    /// Writes the spans as WebVTT with one empty voice-tagged cue each (`<v Name></v>`).
    ///
    /// Outputs: The document text. Side effects: None.
    pub fn to_vtt(&self) -> String {
        let mut text = String::from("WEBVTT\n");
        for span in self.spans() {
            text.push_str(&format!(
                "\n{} --> {}\n<v {}></v>\n",
                vtt_timestamp(span.start_ms),
                vtt_timestamp(span.end_ms),
                escape_cue_text(&span.name)
            ));
        }
        text
    }
}

/// Formats milliseconds as `HH:MM:SS.mmm`.
fn vtt_timestamp(milliseconds: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        milliseconds / 3_600_000,
        milliseconds / 60_000 % 60,
        milliseconds / 1_000 % 60,
        milliseconds % 1_000
    )
}

/// Escapes `&`, `<` and `>` for a cue payload.
fn escape_cue_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Reverses `escape_cue_text`.
fn unescape_cue_text(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Parses `HH:MM:SS.mmm` or `MM:SS.mmm` into milliseconds.
fn parse_vtt_timestamp(text: &str) -> Option<u64> {
    let (clock, millis) = text.trim().split_once('.')?;
    let millis: u64 = millis.parse().ok()?;
    let fields: Vec<u64> = clock
        .split(':')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let (hours, minutes, seconds) = match fields.as_slice() {
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        [minutes, seconds] => (0, *minutes, *seconds),
        _ => return None,
    };
    Some(((hours * 60 + minutes) * 60 + seconds) * 1_000 + millis)
}

/// Reads the voice-tagged cues of a WebVTT document.
///
/// Inputs: WebVTT text (window readings, Teams captions or the live transcript, #102).
/// Outputs: One span per cue whose payload starts with a `<v Name>` tag (a class such as
/// `<v.loud Name>` is ignored); cues without one, and malformed timings, are skipped.
/// Side effects: None.
pub fn parse_voice_cues(text: &str) -> Vec<NamedSpan> {
    let mut spans = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let Some((start_text, rest)) = line.split_once("-->") else {
            continue;
        };
        let end_text = rest.split_whitespace().next().unwrap_or_default();
        let (Some(start_ms), Some(end_ms)) = (
            parse_vtt_timestamp(start_text),
            parse_vtt_timestamp(end_text),
        ) else {
            continue;
        };
        let Some(payload) = lines.next() else {
            break;
        };
        if let Some(name) = voice_tag_name(payload) {
            spans.push(NamedSpan::new(start_ms, end_ms.max(start_ms), &name));
        }
    }
    spans
}

/// Returns the name of a payload's leading `<v Name>` tag.
fn voice_tag_name(payload: &str) -> Option<String> {
    let tag = payload.trim_start().strip_prefix("<v")?;
    let (inside, _) = tag.split_once('>')?;
    // `<v.class Name>`: skip the classes up to the first space.
    let name = if inside.starts_with('.') {
        inside.split_once(' ')?.1
    } else {
        inside.strip_prefix(' ')?
    };
    plausible_name(&unescape_cue_text(name))
}

/// Suggests a name for each unknown voice from the names shown while it spoke.
///
/// Inputs: The unknown voices' turns and the named spans (window readings and captions).
/// Outputs: For each voice with at least `MIN_EVIDENCE_MS` of speech under named spans, the name
/// covering at least `MIN_NAME_SHARE` of that speech. Names are compared ignoring case and
/// spacing; the spelling with the most overlap is returned. Side effects: None.
pub fn suggest_names(turns: &[VoiceTurn], spans: &[NamedSpan]) -> HashMap<Uuid, String> {
    /// Overlap per voice, then per compared name: total milliseconds and per spelling.
    type SpellingOverlap = Vec<(String, u64)>;
    let mut overlap: HashMap<Uuid, Vec<(String, u64, SpellingOverlap)>> = HashMap::new();
    for turn in turns {
        for span in spans {
            let shared_ms = turn
                .end_ms
                .min(span.end_ms)
                .saturating_sub(turn.start_ms.max(span.start_ms));
            if shared_ms == 0 {
                continue;
            }
            let spelling = span.name.split_whitespace().collect::<Vec<_>>().join(" ");
            let key = spelling.to_lowercase();
            let names = overlap.entry(turn.cluster_id).or_default();
            let entry = match names.iter_mut().position(|(known, _, _)| *known == key) {
                Some(index) => &mut names[index],
                None => {
                    names.push((key, 0, Vec::new()));
                    let last = names.len() - 1;
                    &mut names[last]
                }
            };
            entry.1 += shared_ms;
            match entry.2.iter_mut().find(|(known, _)| *known == spelling) {
                Some((_, total)) => *total += shared_ms,
                None => entry.2.push((spelling, shared_ms)),
            }
        }
    }
    let mut suggestions = HashMap::new();
    for (cluster_id, names) in overlap {
        let named_ms: u64 = names.iter().map(|(_, total, _)| total).sum();
        let Some((_, best_ms, spellings)) = names
            .iter()
            .reduce(|best, next| if next.1 > best.1 { next } else { best })
        else {
            continue;
        };
        // Integer milliseconds are far below f64's exact range.
        #[allow(clippy::cast_precision_loss)]
        let share = *best_ms as f64 / named_ms as f64;
        if named_ms < MIN_EVIDENCE_MS || share < MIN_NAME_SHARE {
            continue;
        }
        if let Some((spelling, _)) = spellings
            .iter()
            .reduce(|best, next| if next.1 > best.1 { next } else { best })
        {
            suggestions.insert(cluster_id, spelling.clone());
        }
    }
    suggestions
}
