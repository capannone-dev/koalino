//! Judges whether an online-meeting participant's audio track carries one voice or several (#103).
//!
//! A meeting platform sends one audio track per participant, and usually one person sits behind
//! it. In a hybrid meeting, though, a meeting room joins as one participant with several people
//! behind its microphone, and a shared laptop does the same. Crediting every word of such a track
//! to the participant's name is wrong, and keeping its blended voice in the gallery would spoil
//! later matching.
//!
//! So every chunk of a track is diarized, and:
//! - `TrackVoicePolicy::classify_chunk` groups the chunk's turns by voice and says whether one
//!   voice clearly dominates (`OneVoice`) or several really speak (`SeveralVoices`);
//! - `ParticipantTrackVoices::observe` follows the track across chunks. It keeps the
//!   participant's name while one voice speaks (`TrackVerdict::Participant`). Once several voices
//!   appear, in one chunk or as a different voice in a later chunk, it tells them apart for the
//!   rest of the meeting (`TrackVerdict::SeparateVoices`); the session then matches each voice
//!   against the gallery like local audio.
//! - `enrolment_candidate` offers the track's voice for the gallery once, after enough clean
//!   speech of the one voice, and never for a track with several voices. Keeping it is still
//!   opt-in (see `IdentityMapper::keep_participant_voice`).
//!
//! Chunks credited to the participant before a second voice appeared stay credited. Embeddings
//! are biometric and are never logged. Everything here is pure.

use crate::domain::transcript_confidence::OverlapSpan;

/// A voice must hold at least this share of a chunk's speech to count as the only voice.
pub const DOMINANT_VOICE_SHARE: f32 = 0.9;
/// Other voices with less speech than this in a chunk are ignored as noise or cross-talk.
pub const MINORITY_VOICE_FLOOR_MS: u64 = 2_000;
/// Two turns are the same voice when their cosine similarity is above this. It matches the
/// unknown-speaker clustering cut-off, so a track splits where the app would see two people.
pub const SAME_VOICE_SIMILARITY: f32 = 0.40;
/// A later chunk is compared with the track's voice only after this much speech of it.
pub const KNOWN_VOICE_AFTER_MS: u64 = 5_000;
/// Clean speech of the one voice needed before it is offered for the gallery.
pub const ENROL_AFTER_CLEAN_MS: u64 = 30_000;

/// One diarized turn of a chunk, relative to the chunk.
#[derive(Clone, Copy, Debug)]
pub struct VoiceTurn<'embedding> {
    /// Start in chunk milliseconds.
    pub start_ms: u64,
    /// End in chunk milliseconds.
    pub end_ms: u64,
    /// The turn's voice embedding; biometric, never logged.
    pub embedding: &'embedding [f32],
}

impl VoiceTurn<'_> {
    /// The turn's length in milliseconds. Side effects: None.
    fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }
}

/// What one chunk of a participant track holds.
#[derive(Clone, Debug, PartialEq)]
pub enum ChunkVoices {
    /// No speech was found.
    Silent,
    /// One voice clearly dominates.
    OneVoice {
        /// The dominant voice's speech in the chunk.
        speech_ms: u64,
        /// The part of it with no overlapping voice.
        clean_ms: u64,
        /// The dominant voice's mean embedding, weighted by turn length; biometric.
        embedding: Vec<f32>,
    },
    /// Two or more voices each speak for real.
    SeveralVoices,
}

/// How a chunk of a participant track is credited.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackVerdict {
    /// One person: the whole chunk goes to the participant.
    Participant,
    /// Several people: each voice is matched against the gallery on its own.
    SeparateVoices,
}

/// The thresholds of the judgement; `Default` holds the module's constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackVoicePolicy {
    /// See `DOMINANT_VOICE_SHARE`.
    pub dominant_share: f32,
    /// See `MINORITY_VOICE_FLOOR_MS`.
    pub minority_floor_ms: u64,
    /// See `SAME_VOICE_SIMILARITY`.
    pub same_voice_similarity: f32,
    /// See `KNOWN_VOICE_AFTER_MS`.
    pub known_voice_after_ms: u64,
    /// See `ENROL_AFTER_CLEAN_MS`.
    pub enrol_after_clean_ms: u64,
}

impl Default for TrackVoicePolicy {
    /// The module's constants. Side effects: None.
    fn default() -> Self {
        Self {
            dominant_share: DOMINANT_VOICE_SHARE,
            minority_floor_ms: MINORITY_VOICE_FLOOR_MS,
            same_voice_similarity: SAME_VOICE_SIMILARITY,
            known_voice_after_ms: KNOWN_VOICE_AFTER_MS,
            enrol_after_clean_ms: ENROL_AFTER_CLEAN_MS,
        }
    }
}

/// Turns grouped as one voice within a chunk.
struct VoiceGroup<'embedding> {
    /// Running mean embedding, weighted by speech.
    centroid: WeightedMean,
    /// The group's turns.
    turns: Vec<VoiceTurn<'embedding>>,
}

impl TrackVoicePolicy {
    /// Groups a chunk's diarized turns by voice and judges the chunk.
    ///
    /// Inputs: `turns` from diarization, relative to the chunk, each with its embedding;
    /// `overlap` the chunk-relative stretches where voices overlap.
    /// Outputs: `Silent` without speech; `OneVoice` when one voice holds at least
    /// `dominant_share` of the speech or the others together stay under `minority_floor_ms`;
    /// otherwise `SeveralVoices`. A turn whose embedding cannot be compared starts its own group.
    /// Side effects: None.
    pub fn classify_chunk(&self, turns: &[VoiceTurn<'_>], overlap: &[OverlapSpan]) -> ChunkVoices {
        let mut groups: Vec<VoiceGroup<'_>> = Vec::new();
        for voice_turn in turns
            .iter()
            .filter(|voice_turn| voice_turn.duration_ms() > 0)
        {
            let matching_group = groups.iter_mut().find(|group| {
                cosine(group.centroid.value(), voice_turn.embedding)
                    .is_some_and(|similarity| similarity > self.same_voice_similarity)
            });
            match matching_group {
                Some(group) => {
                    group
                        .centroid
                        .add(voice_turn.embedding, voice_turn.duration_ms());
                    group.turns.push(*voice_turn);
                }
                None => {
                    let mut centroid = WeightedMean::default();
                    centroid.add(voice_turn.embedding, voice_turn.duration_ms());
                    groups.push(VoiceGroup {
                        centroid,
                        turns: vec![*voice_turn],
                    });
                }
            }
        }
        let total_ms: u64 = groups.iter().map(|group| group.centroid.weight).sum();
        let Some(dominant) = groups.iter().max_by_key(|group| group.centroid.weight) else {
            return ChunkVoices::Silent;
        };
        let dominant_ms = dominant.centroid.weight;
        let others_ms = total_ms.saturating_sub(dominant_ms);
        // Lossy only above 2^24 ms (4.6 hours), far beyond a 30 s chunk.
        #[allow(clippy::cast_precision_loss)]
        let dominant_share = dominant_ms as f32 / total_ms as f32;
        if dominant_share < self.dominant_share && others_ms >= self.minority_floor_ms {
            return ChunkVoices::SeveralVoices;
        }
        let overlapped_ms: u64 = dominant
            .turns
            .iter()
            .map(|voice_turn| overlap_within(voice_turn, overlap))
            .sum();
        ChunkVoices::OneVoice {
            speech_ms: dominant_ms,
            clean_ms: dominant_ms.saturating_sub(overlapped_ms),
            embedding: dominant.centroid.value().to_vec(),
        }
    }
}

/// Follows one participant's track through a meeting.
#[derive(Clone, Debug, Default)]
pub struct ParticipantTrackVoices {
    /// The track's one voice so far, weighted by speech.
    voice: WeightedMean,
    /// Clean speech of the one voice so far.
    clean_ms: u64,
    /// Set once several voices were heard; never cleared.
    several_voices: bool,
    /// Set once the voice was offered for the gallery.
    enrolment_settled: bool,
}

impl ParticipantTrackVoices {
    /// Takes in one chunk's judgement and says how to credit the chunk.
    ///
    /// Inputs: The policy and the chunk's `ChunkVoices`.
    /// Outputs: `SeparateVoices` for a chunk with several voices, for a single voice unlike the
    /// track's known voice, and for every chunk after either; otherwise `Participant`.
    /// Side effects: Updates the track's voice and clean speech, or marks it as several voices.
    pub fn observe(&mut self, policy: &TrackVoicePolicy, voices: ChunkVoices) -> TrackVerdict {
        if self.several_voices {
            return TrackVerdict::SeparateVoices;
        }
        match voices {
            ChunkVoices::Silent => TrackVerdict::Participant,
            ChunkVoices::SeveralVoices => self.mark_several_voices(),
            ChunkVoices::OneVoice {
                speech_ms,
                clean_ms,
                embedding,
            } => {
                if self.voice.weight >= policy.known_voice_after_ms
                    && cosine(self.voice.value(), &embedding)
                        .is_some_and(|similarity| similarity <= policy.same_voice_similarity)
                {
                    return self.mark_several_voices();
                }
                if self.voice.accepts(&embedding) {
                    self.voice.add(&embedding, speech_ms);
                    self.clean_ms = self.clean_ms.saturating_add(clean_ms);
                }
                TrackVerdict::Participant
            }
        }
    }

    /// Offers the track's voice for the gallery.
    ///
    /// Inputs: The policy.
    /// Outputs: The voice's mean embedding once one voice gave `enrol_after_clean_ms` of clean
    /// speech and it was not offered before; `None` otherwise and always for several voices.
    /// Side effects: None; call `settle_enrolment` once the offer was handled.
    pub fn enrolment_candidate(&self, policy: &TrackVoicePolicy) -> Option<Vec<f32>> {
        (!self.several_voices
            && !self.enrolment_settled
            && self.clean_ms >= policy.enrol_after_clean_ms)
            .then(|| self.voice.value().to_vec())
    }

    /// Records that the voice was offered, so it is not offered again this meeting.
    /// Side effects: Updates the track.
    pub fn settle_enrolment(&mut self) {
        self.enrolment_settled = true;
    }

    /// Whether the track was found to carry several voices. Side effects: None.
    pub fn has_several_voices(&self) -> bool {
        self.several_voices
    }

    /// Marks the track as several voices. Outputs: `SeparateVoices`. Side effects: Updates it.
    fn mark_several_voices(&mut self) -> TrackVerdict {
        self.several_voices = true;
        TrackVerdict::SeparateVoices
    }
}

/// A mean of embeddings weighted by milliseconds of speech.
#[derive(Clone, Debug, Default)]
struct WeightedMean {
    /// The current mean; empty before the first sample.
    mean: Vec<f32>,
    /// Total weight in milliseconds.
    weight: u64,
}

impl WeightedMean {
    /// The current mean. Side effects: None.
    fn value(&self) -> &[f32] {
        &self.mean
    }

    /// Whether `embedding` can join: the first sample, or one of the same length.
    /// Side effects: None.
    fn accepts(&self, embedding: &[f32]) -> bool {
        self.mean.is_empty() || self.mean.len() == embedding.len()
    }

    /// Adds `embedding` with `weight_ms`; a sample of another length is ignored.
    /// Side effects: Updates the mean.
    fn add(&mut self, embedding: &[f32], weight_ms: u64) {
        if weight_ms == 0 || !self.accepts(embedding) {
            return;
        }
        if self.mean.is_empty() {
            self.mean = embedding.to_vec();
            self.weight = weight_ms;
            return;
        }
        let total = self.weight.saturating_add(weight_ms);
        // Speech totals stay far below 2^24 ms per meeting track in practice; the ratio only
        // steers a mean, so the rounding is harmless.
        #[allow(clippy::cast_precision_loss)]
        let new_share = weight_ms as f32 / total as f32;
        for (mean_value, sample_value) in self.mean.iter_mut().zip(embedding) {
            *mean_value += (sample_value - *mean_value) * new_share;
        }
        self.weight = total;
    }
}

/// Milliseconds of `voice_turn` inside any overlap span. Side effects: None.
fn overlap_within(voice_turn: &VoiceTurn<'_>, overlap: &[OverlapSpan]) -> u64 {
    overlap
        .iter()
        .map(|span| {
            let start = span.start_ms.max(voice_turn.start_ms);
            let end = span.end_ms.min(voice_turn.end_ms);
            end.saturating_sub(start)
        })
        .sum()
}

/// Cosine similarity of two embeddings.
///
/// Outputs: `None` for empty vectors, different lengths or a zero norm. Side effects: None.
fn cosine(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.is_empty() || left.len() != right.len() {
        return None;
    }
    let (dot, left_norm, right_norm) = left
        .iter()
        .zip(right)
        .fold((0.0_f32, 0.0_f32, 0.0_f32), |(dot, l, r), (a, b)| {
            (dot + a * b, l + a * a, r + b * b)
        });
    let norms = left_norm.sqrt() * right_norm.sqrt();
    (norms > 0.0 && norms.is_finite()).then(|| dot / norms)
}
