//! Aligns diarized speaker regions with VAD chunk boundaries and distributes transcript text.
//!
//! The VAD chunk is the alignment reference: diarization regions are clamped to the chunk, overlaps
//! and gaps are resolved so regions tile the chunk exactly, and adjacent regions of one speaker are
//! merged (keeping sentences together). Each word then goes to the region at its midpoint when the
//! engine reports word timing. Engines without timing get words in proportion to region duration.
//! Everything here is pure and side-effect free.

use uuid::Uuid;

use crate::domain::models::{TimedWord, Transcript};

/// Reports VAD boundaries whose end precedes their start.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("chunk end {end_offset_ms} ms precedes chunk start {start_offset_ms} ms")]
pub struct ChunkBoundsError {
    /// The rejected meeting-relative start offset.
    pub start_offset_ms: u64,
    /// The rejected meeting-relative end offset.
    pub end_offset_ms: u64,
}

/// Meeting-relative VAD chunk boundaries used as the primary alignment reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkBounds {
    start_offset_ms: u64,
    end_offset_ms: u64,
}

impl ChunkBounds {
    /// Creates validated chunk boundaries.
    ///
    /// Inputs: Offsets in milliseconds from the meeting start; `end_offset_ms` must not precede
    /// `start_offset_ms`.
    /// Outputs: The boundaries, or `ChunkBoundsError` when they are inverted.
    /// Side effects: None.
    pub fn new(start_offset_ms: u64, end_offset_ms: u64) -> Result<Self, ChunkBoundsError> {
        if end_offset_ms < start_offset_ms {
            return Err(ChunkBoundsError {
                start_offset_ms,
                end_offset_ms,
            });
        }
        Ok(Self {
            start_offset_ms,
            end_offset_ms,
        })
    }

    /// Returns the chunk start offset from the meeting start in milliseconds.
    pub fn start_offset_ms(&self) -> u64 {
        self.start_offset_ms
    }

    /// Returns the chunk end offset from the meeting start in milliseconds.
    pub fn end_offset_ms(&self) -> u64 {
        self.end_offset_ms
    }

    /// Returns the chunk duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.end_offset_ms - self.start_offset_ms
    }
}

/// Describes how a transcript interval was (or could not be) attributed to a speaker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpeakerAttribution {
    /// A strong voice match or a guaranteed external identity.
    Known {
        /// The attributed local speaker identity.
        identity_id: Uuid,
    },
    /// A weak voice match awaiting user confirmation in the Voice Gallery.
    PendingReview {
        /// The preserved review clip, or `None` when the clip could not be stored or the voice
        /// joined an existing cluster (whose founding clip already represents it).
        clip_id: Option<Uuid>,
        /// The weak candidate suggested to the user.
        candidate_identity_id: Uuid,
        /// The unknown-speaker cluster the voice was grouped into, if clustering succeeded.
        cluster_id: Option<Uuid>,
    },
    /// No voice match, or identity matching/mapping failed.
    Unknown {
        /// The preserved review clip, or `None` when no clip was (or could be) stored or the
        /// voice joined an existing cluster (whose founding clip already represents it).
        clip_id: Option<Uuid>,
        /// The unknown-speaker cluster the voice was grouped into, if clustering succeeded.
        cluster_id: Option<Uuid>,
    },
    /// Diarization was unavailable or failed, so no speaker analysis was attempted.
    Unattributed,
}

impl SpeakerAttribution {
    /// Returns the user-facing speaker label for an attribution category.
    ///
    /// Inputs: None.
    /// Outputs: `"Known"`, `"Unknown (pending review)"`, `"Unknown"`, or `"Unattributed"`.
    /// Side effects: None.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Known { .. } => "Known",
            Self::PendingReview { .. } => "Unknown (pending review)",
            Self::Unknown { .. } => "Unknown",
            Self::Unattributed => "Unattributed",
        }
    }

    /// Returns the identity persisted with a transcript segment.
    ///
    /// Inputs: None.
    /// Outputs: The identity for `Known`; `None` for every unconfirmed category.
    /// Side effects: None.
    pub fn speaker_identity_id(&self) -> Option<Uuid> {
        match self {
            Self::Known { identity_id } => Some(*identity_id),
            Self::PendingReview { .. } | Self::Unknown { .. } | Self::Unattributed => None,
        }
    }

    /// Returns the unknown-speaker cluster persisted with a transcript segment.
    ///
    /// Inputs: None.
    /// Outputs: The cluster for clustered `PendingReview`/`Unknown` regions; otherwise `None`.
    /// Side effects: None.
    pub fn unknown_cluster_id(&self) -> Option<Uuid> {
        match self {
            Self::PendingReview { cluster_id, .. } | Self::Unknown { cluster_id, .. } => {
                *cluster_id
            }
            Self::Known { .. } | Self::Unattributed => None,
        }
    }
}

/// One attributed diarization region with offsets relative to its VAD chunk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerRegion {
    start_time_ms: u64,
    end_time_ms: u64,
    attribution: SpeakerAttribution,
}

impl SpeakerRegion {
    /// Creates a chunk-relative speaker region.
    ///
    /// Inputs: Offsets in milliseconds from the chunk start, as returned by diarization;
    /// out-of-range or inverted offsets are tolerated and clamped/dropped during alignment.
    /// Outputs: An immutable region.
    /// Side effects: None.
    pub fn new(start_time_ms: u64, end_time_ms: u64, attribution: SpeakerAttribution) -> Self {
        Self {
            start_time_ms,
            end_time_ms,
            attribution,
        }
    }

    /// Borrows the region's speaker attribution, before any words are aligned to it.
    pub fn attribution(&self) -> &SpeakerAttribution {
        &self.attribution
    }
}

/// One aligned, meeting-timed region with its share of the chunk transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlignedRegion {
    start_time_ms: u64,
    end_time_ms: u64,
    attribution: SpeakerAttribution,
    text: String,
}

impl AlignedRegion {
    /// Returns the start offset from the meeting start in milliseconds.
    pub fn start_time_ms(&self) -> u64 {
        self.start_time_ms
    }

    /// Returns the end offset from the meeting start in milliseconds.
    pub fn end_time_ms(&self) -> u64 {
        self.end_time_ms
    }

    /// Borrows the region's speaker attribution.
    pub fn attribution(&self) -> &SpeakerAttribution {
        &self.attribution
    }

    /// Borrows the transcript words assigned to this region.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Aligns diarized regions to a VAD chunk and splits the chunk transcript between them.
///
/// Inputs: `chunk_bounds` are meeting-relative VAD boundaries; `speaker_regions` are
/// chunk-relative diarization regions in any order; `transcript_text` is the chunk transcript.
/// Outputs: Non-empty-text regions in meeting time that tile the chunk in order. Without usable
/// regions the whole chunk becomes one `Unattributed` region; blank text yields no regions.
/// Side effects: None. Whitespace inside the transcript is normalized to single spaces.
pub fn align_chunk_regions(
    chunk_bounds: ChunkBounds,
    speaker_regions: Vec<SpeakerRegion>,
    transcript_text: &str,
) -> Vec<AlignedRegion> {
    let transcript_words: Vec<&str> = transcript_text.split_whitespace().collect();
    if transcript_words.is_empty() {
        return Vec::new();
    }
    let chunk_duration_ms = chunk_bounds.duration_ms();
    let tiled_regions = chunk_tiling(speaker_regions, chunk_duration_ms);

    let total_word_count = transcript_words.len();
    let last_region_index = tiled_regions.len() - 1;
    let mut next_word_index = 0;
    let mut aligned_regions = Vec::with_capacity(tiled_regions.len());
    for (region_index, region) in tiled_regions.into_iter().enumerate() {
        let word_end_index = if region_index == last_region_index {
            total_word_count
        } else {
            proportional_word_index(total_word_count, region.end_time_ms, chunk_duration_ms)
                .max(next_word_index)
        };
        let region_words = &transcript_words[next_word_index..word_end_index];
        next_word_index = word_end_index;
        if region_words.is_empty() {
            continue;
        }
        aligned_regions.push(AlignedRegion {
            start_time_ms: chunk_bounds.start_offset_ms() + region.start_time_ms,
            end_time_ms: chunk_bounds.start_offset_ms() + region.end_time_ms,
            attribution: region.attribution,
            text: region_words.join(" "),
        });
    }
    aligned_regions
}

/// Aligns regions and text, using word timing when the engine reported it.
///
/// Inputs: Meeting-relative chunk bounds, chunk-relative regions, and the chunk's transcript.
/// Outputs: The aligned regions, from `align_timed_words` with timing and from
/// `align_chunk_regions` without it.
/// Side effects: None.
pub fn align_transcript(
    chunk_bounds: ChunkBounds,
    speaker_regions: Vec<SpeakerRegion>,
    transcript: &Transcript,
) -> Vec<AlignedRegion> {
    if transcript.timed_words().is_empty() {
        align_chunk_regions(chunk_bounds, speaker_regions, transcript.content())
    } else {
        align_timed_words(chunk_bounds, speaker_regions, transcript.timed_words())
    }
}

/// Gives each timed word to the region at its midpoint.
///
/// Inputs: Meeting-relative chunk bounds, chunk-relative regions in any order, and chunk-relative
/// words in spoken order.
/// Outputs: Non-empty-text regions in meeting time, in order. A word never moves back to an earlier
/// region, so a timing glitch cannot reorder the text. Without usable regions the whole chunk
/// becomes one `Unattributed` region; no words yield no regions.
/// Side effects: None.
pub fn align_timed_words(
    chunk_bounds: ChunkBounds,
    speaker_regions: Vec<SpeakerRegion>,
    timed_words: &[TimedWord],
) -> Vec<AlignedRegion> {
    let tiled_regions = chunk_tiling(speaker_regions, chunk_bounds.duration_ms());
    let mut region_words: Vec<Vec<&str>> = vec![Vec::new(); tiled_regions.len()];
    let mut current_region_index = 0;
    for timed_word in timed_words {
        let midpoint_ms = timed_word.midpoint_ms();
        while current_region_index + 1 < tiled_regions.len()
            && midpoint_ms >= tiled_regions[current_region_index].end_time_ms
        {
            current_region_index += 1;
        }
        region_words[current_region_index].push(timed_word.text());
    }
    tiled_regions
        .into_iter()
        .zip(region_words)
        .filter(|(_, words)| !words.is_empty())
        .map(|(region, words)| AlignedRegion {
            start_time_ms: chunk_bounds.start_offset_ms() + region.start_time_ms,
            end_time_ms: chunk_bounds.start_offset_ms() + region.end_time_ms,
            attribution: region.attribution,
            text: words.join(" "),
        })
        .collect()
}

/// Turns raw regions into ordered regions that cover the whole chunk.
///
/// Inputs: Raw chunk-relative regions and the chunk duration.
/// Outputs: At least one region, with no gaps; `Unattributed` when none were usable.
/// Side effects: None.
fn chunk_tiling(speaker_regions: Vec<SpeakerRegion>, chunk_duration_ms: u64) -> Vec<SpeakerRegion> {
    let mut tiled_regions = tile_chunk(normalize_regions(speaker_regions, chunk_duration_ms));
    if tiled_regions.is_empty() {
        tiled_regions.push(SpeakerRegion::new(
            0,
            chunk_duration_ms,
            SpeakerAttribution::Unattributed,
        ));
    }
    if let Some(last_region) = tiled_regions.last_mut() {
        last_region.end_time_ms = chunk_duration_ms;
    }
    tiled_regions
}

/// Clamps regions to the chunk, orders them, removes overlaps, and merges same-speaker neighbours.
///
/// Inputs: Raw chunk-relative regions and the chunk duration.
/// Outputs: Ordered, non-overlapping, non-empty regions inside `[0, chunk_duration_ms]`.
/// Side effects: None.
fn normalize_regions(
    speaker_regions: Vec<SpeakerRegion>,
    chunk_duration_ms: u64,
) -> Vec<SpeakerRegion> {
    let mut clamped_regions: Vec<SpeakerRegion> = speaker_regions
        .into_iter()
        .map(|region| SpeakerRegion {
            start_time_ms: region.start_time_ms.min(chunk_duration_ms),
            end_time_ms: region.end_time_ms.min(chunk_duration_ms),
            attribution: region.attribution,
        })
        .filter(|region| region.end_time_ms > region.start_time_ms)
        .collect();
    clamped_regions.sort_by_key(|region| region.start_time_ms);

    let mut normalized_regions: Vec<SpeakerRegion> = Vec::with_capacity(clamped_regions.len());
    for mut region in clamped_regions {
        if let Some(previous_region) = normalized_regions.last_mut() {
            if previous_region.attribution == region.attribution {
                previous_region.end_time_ms = previous_region.end_time_ms.max(region.end_time_ms);
                continue;
            }
            region.start_time_ms = region.start_time_ms.max(previous_region.end_time_ms);
            if region.end_time_ms <= region.start_time_ms {
                continue;
            }
        }
        normalized_regions.push(region);
    }
    normalized_regions
}

/// Extends ordered regions so they cover the chunk with no gaps.
///
/// Inputs: Output of `normalize_regions`.
/// Outputs: Regions where the first starts at 0 and each internal gap is split at its midpoint;
/// the caller sets the final end to the chunk duration.
/// Side effects: None.
fn tile_chunk(mut normalized_regions: Vec<SpeakerRegion>) -> Vec<SpeakerRegion> {
    if let Some(first_region) = normalized_regions.first_mut() {
        first_region.start_time_ms = 0;
    }
    for region_index in 1..normalized_regions.len() {
        let previous_end_ms = normalized_regions[region_index - 1].end_time_ms;
        let current_start_ms = normalized_regions[region_index].start_time_ms;
        let gap_midpoint_ms = previous_end_ms + (current_start_ms - previous_end_ms) / 2;
        normalized_regions[region_index - 1].end_time_ms = gap_midpoint_ms;
        normalized_regions[region_index].start_time_ms = gap_midpoint_ms;
    }
    normalized_regions
}

/// Maps a chunk-relative time to the rounded word index proportional to elapsed duration.
///
/// Inputs: Total words, a boundary offset, and the chunk duration (zero assigns every word).
/// Outputs: A word index in `0..=total_word_count`.
/// Side effects: None.
fn proportional_word_index(
    total_word_count: usize,
    boundary_ms: u64,
    chunk_duration_ms: u64,
) -> usize {
    if chunk_duration_ms == 0 {
        return total_word_count;
    }
    let total_words_wide = total_word_count as u128;
    let rounded_index = (total_words_wide * u128::from(boundary_ms)
        + u128::from(chunk_duration_ms) / 2)
        / u128::from(chunk_duration_ms);
    usize::try_from(rounded_index)
        .unwrap_or(total_word_count)
        .min(total_word_count)
}
