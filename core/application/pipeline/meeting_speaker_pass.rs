//! Decides who spoke when, once the whole meeting is known (#73), and what each speaker's voice is.
//!
//! During a meeting the session collects one voice embedding per short window, and no audio. At the
//! end this module groups all of them. A person's windows are grouped using the whole meeting, so a
//! mistake in one chunk can't last. The functions here are pure, so they can be tested without models.

/// Settings of the final grouping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeakerPassConfig {
    /// Least average cosine similarity at which two groups merge. On the evaluation recordings this
    /// finds every speaker count; 0.21 to 0.3 miss one or two, and 0.15 errs far more (`DECISION_LOG.md`).
    pub similarity_threshold: f32,
    /// A group smaller than this joins its nearest group: about this many seconds of speech. The
    /// least size for short meetings; longer ones raise it, see `min_group_share`.
    pub min_group_windows: usize,
    /// In a long meeting a group also needs this share of all windows, up to
    /// `max_min_group_windows` (#82). One person's voice drifts over a long meeting, so the
    /// grouping splits off small groups of the same person: on the 15 evaluation recordings joined
    /// into one 19-minute meeting, an 8-window minimum found 7 speakers for 3 people and a 30-window
    /// one found 3, with fewer windows misplaced. A 5-minute lunch of 3 people (231 windows) gave 6
    /// speakers at 8 windows, 4 at a 1/15 share (15) and 3 once `min_group_contiguity` also folded
    /// a group of scattered seconds. The price: someone who says less than that is given to the
    /// nearest voice.
    pub min_group_share: f32,
    /// The most windows `min_group_share` can ask a group for: a minute of speech (#61). At half
    /// a minute, real meetings still split one person into several voices (a 3-person, 52-minute
    /// recording gave 8). A minute brings that recording to 4 and lowers speaker confusion on the
    /// AMI and NOTSOFAR sets (11.3% to 9.8% and 8.0% to 6.8%) without losing any real speaker
    /// there; 90 s lowers it further but loses one (`DECISION_LOG.md`).
    pub max_min_group_windows: usize,
    /// A group under `max_min_group_windows` also joins its nearest group when fewer than this
    /// share of its windows have a neighbour of the same group within a second (#83). People talk
    /// in runs: every speaker group of the evaluation recordings scored at least 0.76, and those of
    /// a 5-minute lunch at least 0.88, while the stray group of that lunch, single seconds spread
    /// over the whole meeting, scored 0.53. Used only when window times are given.
    pub min_group_contiguity: f32,
    /// Most windows grouped exhaustively; the rest then join the nearest group. Memory grows with
    /// the square of this count, so it bounds a long meeting at about 18 MB on a phone.
    pub max_clustered_windows: usize,
}

impl Default for SpeakerPassConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.2,
            min_group_windows: 8,
            min_group_share: 1.0 / 15.0,
            max_min_group_windows: 60,
            min_group_contiguity: 0.65,
            max_clustered_windows: 3_000,
        }
    }
}

/// Least similarity to a saved voice for a speaker to be named without asking. On the evaluation
/// recordings right matches reach 0.66 and wrong ones stay at 0.55 or below; a whole speaker's voice is
/// far steadier than one short turn, so this can sit below the per-turn 0.80.
pub const SPEAKER_NAME_THRESHOLD: f32 = 0.60;
/// Least windows, about seconds of speech, behind a voice worth saving.
pub const MIN_CLEAN_VOICE_WINDOWS: usize = 20;
/// Least average similarity of a voice's core windows to their centre for the voice to be saved.
///
/// Measured first on the whole group, this kept out the mixed groups of the 15 test recordings but
/// also almost every real speaker (#196): on AMI, NOTSOFAR and a real Italian meeting, whole groups
/// average 0.45 to 0.60, so only 13 of 121 were saved and 8 of the owner's 17 people had no voice.
/// Measured on the core (`CORE_SHARE`), 103 of 121 are saved; naming the next meeting's speakers with
/// them, right names rose from 8% to 74% of speech and names of the wrong person from 0.7% to 3.8%.
pub const MIN_CLEAN_VOICE_CONSISTENCY: f32 = 0.60;
/// Share of a voice's windows, those nearest its centre, that make up its core: overlap, noise and
/// turn changes scatter the rest. Only a voice with `MIN_CLEAN_VOICE_WINDOWS` has a core; a shorter
/// one is judged on all its windows, since a lone window always looks consistent.
pub const CORE_SHARE: f32 = 0.5;

/// One short window's voice, in meeting time.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceWindow {
    /// The chunk it came from, counted within the session.
    pub chunk_index: usize,
    /// Start in meeting milliseconds.
    pub start_ms: u64,
    /// End in meeting milliseconds.
    pub end_ms: u64,
    /// Derived biometric vector; never logged, and kept only in memory until the meeting ends.
    pub embedding: Vec<f32>,
}

impl VoiceWindow {
    /// Returns the window's middle in meeting milliseconds.
    pub fn center_ms(&self) -> u64 {
        self.start_ms + (self.end_ms.saturating_sub(self.start_ms)) / 2
    }
}

/// What one speaker's windows add up to.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeakerVoice {
    /// The speaker's unit-length average voice.
    pub centroid: Vec<f32>,
    /// How many windows the speaker has.
    pub window_count: usize,
    /// Average similarity of those windows to the centroid.
    pub consistency: f32,
    /// The unit-length average of the core windows (`CORE_SHARE`), the voice that is saved.
    pub core_centroid: Vec<f32>,
    /// Average similarity of the core windows to `core_centroid`.
    pub core_consistency: f32,
}

impl SpeakerVoice {
    /// Says whether this voice is enough, and its core consistent enough, to save for later
    /// matching. The voice to save is then `core_centroid`.
    pub fn is_clean(&self) -> bool {
        self.window_count >= MIN_CLEAN_VOICE_WINDOWS
            && self.core_consistency >= MIN_CLEAN_VOICE_CONSISTENCY
    }
}

/// Summarizes each speaker's voice, and its core.
///
/// Inputs: One embedding and one dense speaker number per window.
/// Outputs: One voice per speaker number. Side effects: None.
pub fn speaker_voices(embeddings: &[Vec<f32>], labels: &[usize]) -> Vec<SpeakerVoice> {
    let vectors: Vec<Vec<f32>> = embeddings
        .iter()
        .map(|vector| unit_vector(vector))
        .collect();
    let speaker_count = labels.iter().max().map_or(0, |highest| highest + 1);
    let centres = group_centres(&vectors, labels, speaker_count);
    centres
        .into_iter()
        .enumerate()
        .map(|(speaker, centroid)| {
            let mut members: Vec<(&Vec<f32>, f32)> = vectors
                .iter()
                .zip(labels)
                .filter(|(_, label)| **label == speaker)
                .map(|(vector, _)| (vector, dot(vector, &centroid)))
                .collect();
            let window_count = members.len();
            let consistency = average(members.iter().map(|(_, similarity)| *similarity));
            if window_count < MIN_CLEAN_VOICE_WINDOWS {
                return SpeakerVoice {
                    core_centroid: centroid.clone(),
                    core_consistency: consistency,
                    centroid,
                    window_count,
                    consistency,
                };
            }
            members.sort_by(|left, right| right.1.total_cmp(&left.1));
            // Rounded up, and at least one window.
            let core_count = ((window_count as f32 * CORE_SHARE).ceil() as usize).max(1);
            let core: Vec<&Vec<f32>> = members
                .iter()
                .take(core_count)
                .map(|(vector, _)| *vector)
                .collect();
            let mut core_sum = vec![0.0_f32; centroid.len()];
            for vector in &core {
                for (total, value) in core_sum.iter_mut().zip(vector.iter()) {
                    *total += value;
                }
            }
            let core_centroid = unit_vector(&core_sum);
            SpeakerVoice {
                core_consistency: average(core.iter().map(|vector| dot(vector, &core_centroid))),
                core_centroid,
                centroid,
                window_count,
                consistency,
            }
        })
        .collect()
}

/// Finds the speaker for a moment: the nearest window in its chunk, else the nearest in the meeting.
///
/// Inputs: Windows in time order with their speakers, a chunk index, and a meeting time.
/// Outputs: The speaker, or `None` without windows. Side effects: None.
pub fn speaker_at(
    windows: &[VoiceWindow],
    labels: &[usize],
    chunk_index: usize,
    time_ms: u64,
) -> Option<usize> {
    let chunk_start = windows.partition_point(|window| window.chunk_index < chunk_index);
    let chunk_end = windows.partition_point(|window| window.chunk_index <= chunk_index);
    let (first, last) = if chunk_start < chunk_end {
        (chunk_start, chunk_end)
    } else {
        (0, windows.len())
    };
    let scope = windows.get(first..last)?;
    let after = scope.partition_point(|window| window.center_ms() < time_ms);
    let distance = |position: usize| scope[position].center_ms().abs_diff(time_ms);
    let nearest = match (after.checked_sub(1), (after < scope.len()).then_some(after)) {
        (Some(before), Some(next)) => {
            if distance(before) <= distance(next) {
                before
            } else {
                next
            }
        }
        (Some(only), None) | (None, Some(only)) => only,
        (None, None) => return None,
    };
    labels.get(first + nearest).copied()
}

/// Returns `vector` scaled to unit length.
///
/// Inputs: Any vector. Outputs: The unit vector, or a copy when its length is zero.
/// Side effects: None.
pub fn unit_vector(vector: &[f32]) -> Vec<f32> {
    let length = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if length > 0.0 {
        vector.iter().map(|value| value / length).collect()
    } else {
        vector.to_vec()
    }
}

/// Returns the mean of `values`, or 0 when there are none.
fn average(values: impl Iterator<Item = f32>) -> f32 {
    let (sum, count) = values.fold((0.0_f32, 0_usize), |(sum, count), value| {
        (sum + value, count + 1)
    });
    sum / count.max(1) as f32
}

/// Returns the dot product, which is the cosine similarity of unit vectors.
fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

/// Returns the position of pair (`low`, `high`), `low < high`, in a triangular distance table.
fn pair_index(low: usize, high: usize, count: usize) -> usize {
    low * count - low * (low + 1) / 2 + high - low - 1
}

/// Groups unit vectors by average linkage, merging while the average similarity reaches the threshold.
///
/// Inputs: Unit vectors and the similarity threshold.
/// Outputs: One group number per vector, not yet dense. Uses the nearest-neighbour chain, which gives
/// the same groups as the textbook algorithm in quadratic time. Side effects: None.
pub(crate) fn average_linkage(vectors: &[Vec<f32>], similarity_threshold: f32) -> Vec<usize> {
    let count = vectors.len();
    let mut distances = vec![0.0_f32; count * count.saturating_sub(1) / 2];
    for low in 0..count {
        for high in low + 1..count {
            distances[pair_index(low, high, count)] = 1.0 - dot(&vectors[low], &vectors[high]);
        }
    }
    let mut sizes = vec![1_usize; count];
    let mut active = vec![true; count];
    let mut merges: Vec<(f32, usize, usize)> = Vec::with_capacity(count.saturating_sub(1));
    let mut chain: Vec<usize> = Vec::new();
    let distance_between = |distances: &[f32], left: usize, right: usize| {
        distances[pair_index(left.min(right), left.max(right), count)]
    };
    for _ in 1..count {
        if chain.is_empty()
            && let Some(first_active) = active.iter().position(|is_active| *is_active)
        {
            chain.push(first_active);
        }
        while let Some(&tip) = chain.last() {
            let previous = chain.len().checked_sub(2).map(|position| chain[position]);
            // Preferring the previous link on a tie guarantees that the chain ends.
            let mut nearest = previous;
            let mut nearest_distance = previous.map_or(f32::INFINITY, |item| {
                distance_between(&distances, tip, item)
            });
            for (candidate, is_active) in active.iter().enumerate() {
                if *is_active && candidate != tip {
                    let distance = distance_between(&distances, tip, candidate);
                    if distance < nearest_distance {
                        nearest = Some(candidate);
                        nearest_distance = distance;
                    }
                }
            }
            let Some(nearest) = nearest else { break };
            if Some(nearest) != previous {
                chain.push(nearest);
                continue;
            }
            chain.pop();
            chain.pop();
            merges.push((nearest_distance, tip.min(nearest), tip.max(nearest)));
            let (kept, removed) = (tip.min(nearest), tip.max(nearest));
            let (kept_size, removed_size) = (sizes[kept] as f32, sizes[removed] as f32);
            for (other, is_active) in active.iter().enumerate() {
                if *is_active && other != kept && other != removed {
                    let kept_slot = pair_index(kept.min(other), kept.max(other), count);
                    let removed_slot = pair_index(removed.min(other), removed.max(other), count);
                    distances[kept_slot] = (kept_size * distances[kept_slot]
                        + removed_size * distances[removed_slot])
                        / (kept_size + removed_size);
                }
            }
            active[removed] = false;
            sizes[kept] += sizes[removed];
            break;
        }
    }
    merges.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut parents: Vec<usize> = (0..count).collect();
    /// Finds the root of `item`, shortening the path as it goes.
    fn find(parents: &mut [usize], mut item: usize) -> usize {
        while parents[item] != item {
            parents[item] = parents[parents[item]];
            item = parents[item];
        }
        item
    }
    for (distance, left, right) in merges {
        if distance > 1.0 - similarity_threshold {
            break;
        }
        let (left_root, right_root) = (find(&mut parents, left), find(&mut parents, right));
        parents[left_root] = right_root;
    }
    (0..count).map(|item| find(&mut parents, item)).collect()
}

/// Returns each group's unit-length centre.
///
/// Inputs: Unit vectors and their group numbers, below `group_count`.
/// Outputs: One centre per group; empty groups have an empty centre. Side effects: None.
fn group_centres(vectors: &[Vec<f32>], labels: &[usize], group_count: usize) -> Vec<Vec<f32>> {
    let mut sums: Vec<Vec<f32>> = vec![Vec::new(); group_count];
    for (vector, label) in vectors.iter().zip(labels) {
        let sum = &mut sums[*label];
        if sum.is_empty() {
            sum.clone_from(vector);
        } else {
            for (total, value) in sum.iter_mut().zip(vector) {
                *total += value;
            }
        }
    }
    sums.iter().map(|sum| unit_vector(sum)).collect()
}

/// Returns the group whose centre is nearest `vector`, among groups for which `allowed` holds.
fn nearest_group(
    vector: &[f32],
    centres: &[Vec<f32>],
    allowed: impl Fn(usize) -> bool,
) -> Option<usize> {
    centres
        .iter()
        .enumerate()
        .filter(|(group, centre)| !centre.is_empty() && allowed(*group))
        .max_by(|(_, left), (_, right)| dot(vector, left).total_cmp(&dot(vector, right)))
        .map(|(group, _)| group)
}

/// Makes each group that is too small, or a brief group heard only in isolated seconds, join the
/// group nearest each of its windows, smallest first.
///
/// Inputs: Unit vectors, their group numbers, each window's start in milliseconds in time order
/// (`None` skips the isolated-seconds rule), the least size, and the settings.
/// Outputs: Updates `labels`; a lone group stays whatever its size. Side effects: None.
fn absorb_small_groups(
    vectors: &[Vec<f32>],
    labels: &mut [usize],
    start_times_ms: Option<&[u64]>,
    min_group_windows: usize,
    config: &SpeakerPassConfig,
) {
    let group_count = labels.iter().max().map_or(0, |highest| highest + 1);
    loop {
        let mut sizes = vec![0_usize; group_count];
        for label in labels.iter() {
            sizes[*label] += 1;
        }
        if sizes.iter().filter(|size| **size > 0).count() <= 1 {
            return;
        }
        let contiguity = start_times_ms.map(|starts| group_contiguity(labels, starts, group_count));
        let smallest = sizes
            .iter()
            .enumerate()
            .filter(|(group, size)| {
                **size > 0
                    && (**size < min_group_windows
                        || (**size < config.max_min_group_windows
                            && contiguity.as_ref().is_some_and(|shares| {
                                shares[*group] < config.min_group_contiguity
                            })))
            })
            .min_by_key(|(_, size)| **size)
            .map(|(group, _)| group);
        let Some(smallest) = smallest else { return };
        let centres = group_centres(vectors, labels, group_count);
        for (vector, label) in vectors.iter().zip(labels.iter_mut()) {
            if *label == smallest
                && let Some(nearest) = nearest_group(vector, &centres, |group| group != smallest)
            {
                *label = nearest;
            }
        }
    }
}

/// Returns, per group, the share of its windows that have another window of the group starting
/// within a second of their own.
///
/// Inputs: Group numbers below `group_count` and each window's start, in time order.
/// Outputs: One share per group; an empty group scores 1. Side effects: None.
fn group_contiguity(labels: &[usize], start_times_ms: &[u64], group_count: usize) -> Vec<f32> {
    const NEIGHBOUR_MS: u64 = 1_000;
    let mut last_start: Vec<Option<u64>> = vec![None; group_count];
    let mut joined = vec![false; labels.len()];
    let mut last_index: Vec<Option<usize>> = vec![None; group_count];
    for (index, (label, start_ms)) in labels.iter().zip(start_times_ms).enumerate() {
        if let (Some(previous_start), Some(previous_index)) =
            (last_start[*label], last_index[*label])
            && start_ms.saturating_sub(previous_start) <= NEIGHBOUR_MS
        {
            joined[index] = true;
            joined[previous_index] = true;
        }
        last_start[*label] = Some(*start_ms);
        last_index[*label] = Some(index);
    }
    let mut totals = vec![(0_usize, 0_usize); group_count];
    for (label, is_joined) in labels.iter().zip(&joined) {
        totals[*label].0 += usize::from(*is_joined);
        totals[*label].1 += 1;
    }
    totals
        .into_iter()
        .map(|(joined_count, count)| {
            if count == 0 {
                1.0
            } else {
                joined_count as f32 / count as f32
            }
        })
        .collect()
}

/// Renumbers groups from zero in order of first appearance.
pub(crate) fn dense_labels(labels: &[usize]) -> Vec<usize> {
    let mut seen: Vec<usize> = Vec::new();
    labels
        .iter()
        .map(|label| {
            seen.iter()
                .position(|known| known == label)
                .unwrap_or_else(|| {
                    seen.push(*label);
                    seen.len() - 1
                })
        })
        .collect()
}

/// Returns how many windows a group needs to stand as its own speaker in a meeting of `count`.
///
/// Inputs: The number of windows grouped and the settings. Outputs: `min_group_windows` for a
/// short meeting, rising with `min_group_share` up to `max_min_group_windows` (never below the
/// least). Side effects: None.
fn least_group_windows(count: usize, config: &SpeakerPassConfig) -> usize {
    let share_windows = (count as f32 * config.min_group_share).floor();
    let share_windows = if share_windows.is_finite() && share_windows > 0.0 {
        share_windows as usize
    } else {
        0
    };
    share_windows
        .min(config.max_min_group_windows)
        .max(config.min_group_windows)
}

/// Groups a meeting's voice windows into speakers.
///
/// Inputs: One embedding per window, in any scale, all the same length.
/// Outputs: One speaker number per window, from zero; empty for no windows. A meeting over
/// `max_clustered_windows` is sampled evenly, and the other windows join the nearest speaker.
/// Side effects: None; CPU-heavy on long meetings, so call it off the UI and async threads.
pub fn group_voice_windows(
    embeddings: &[Vec<f32>],
    start_times_ms: Option<&[u64]>,
    config: &SpeakerPassConfig,
) -> Vec<usize> {
    let vectors: Vec<Vec<f32>> = embeddings
        .iter()
        .map(|vector| unit_vector(vector))
        .collect();
    if vectors.is_empty() {
        return Vec::new();
    }
    let sample_count = vectors.len().min(config.max_clustered_windows.max(1));
    let sampled: Vec<usize> = (0..sample_count)
        .map(|position| position * vectors.len() / sample_count)
        .collect();
    let sample_vectors: Vec<Vec<f32>> = sampled.iter().map(|item| vectors[*item].clone()).collect();
    let mut sample_labels = average_linkage(&sample_vectors, config.similarity_threshold);
    // Sampling spreads a long meeting's windows apart, so their times say nothing about runs.
    let sample_starts = start_times_ms
        .filter(|starts| sample_count == vectors.len() && starts.len() == sample_count);
    absorb_small_groups(
        &sample_vectors,
        &mut sample_labels,
        sample_starts,
        least_group_windows(sample_count, config),
        config,
    );
    let sample_labels = dense_labels(&sample_labels);
    if sample_count == vectors.len() {
        return sample_labels;
    }
    let group_count = sample_labels.iter().max().map_or(0, |highest| highest + 1);
    let centres = group_centres(&sample_vectors, &sample_labels, group_count);
    let labels: Vec<usize> = vectors
        .iter()
        .map(|vector| nearest_group(vector, &centres, |_| true).unwrap_or(0))
        .collect();
    dense_labels(&labels)
}
