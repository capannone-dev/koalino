//! Folds devices' recordings of one meeting together and decides what each device does with its
//! retained speech afterwards (#88).
//!
//! The owner may record a meeting on the laptop and on phones placed near the other people. All
//! recordings sync as ordinary meetings; `find_combinations` recognises them
//! (`same_meeting::recognise_same_meeting`) and folds them into one: the earliest recording, or
//! the first recording of a meeting already folded, so any number of devices join one group. A
//! recording joins through whichever recording of the group it resembles, since a phone at the
//! far end of a table may share words only with another phone. Only a device that recorded one of
//! the two compared decides, so a device that recorded neither never does.
//!
//! One device combines the group's speech (`combining_device`): the first one that is not a phone,
//! or the first phone when all are. Every recording notes whether a phone made it, so every device
//! reaches the same answer from the synced meetings; meetings recorded before that keep the choice
//! written on their first fold.
//!
//! `plan_retained_audio` then says what to do with one meeting's sealed speech (#87):
//! - after `COMBINING_GRACE`, a recording takes the pass alone, so it has a better transcript
//!   soon, and with P2P keeps its sealed speech for up to `KEEP_FOR_COMBINING` in case another
//!   device's recording of it turns up later;
//! - a device that does not combine sends its recording to the one that does, for at most
//!   `KEEP_FOR_COMBINING`;
//! - the combining device combines as soon as every recording's speech is here, however late it
//!   arrives within `KEEP_FOR_COMBINING`, then deletes it all.
//!
//! `MeetingListing` says how folded meetings are shown: once, under the first recording, as
//! recorded on every device that took part; `combining_status` says how far combining got.
//!
//! Everything here is pure; the after-meeting worker carries the plans out.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use chrono::{DateTime, Utc};

use crate::{
    application::same_meeting::{RecordingText, recognise_same_meeting},
    domain::{
        errors::StorageError,
        models::{DeviceId, MeetingId, TranscriptSegment},
    },
    ports::{
        meeting_document::MeetingCombination,
        storage::{MeetingRecording, NoteRepository},
    },
};

/// How long a finished recording waits for another device's recording of it to sync here before
/// it takes the pass alone.
pub const COMBINING_GRACE: Duration = Duration::from_secs(10 * 60);
/// Longest a recording's speech is kept waiting to be combined: on a device that has to send it
/// but cannot reach the combining device, on a device without the pass, or on the combining device
/// for a recording that has not arrived. Three weeks, so a phone that meets the laptop again
/// after a trip still delivers its speech; it stays sealed meanwhile (#87).
pub const KEEP_FOR_COMBINING: Duration = Duration::from_secs(21 * 24 * 60 * 60);
/// Recordings started further apart than this are never compared.
const MAX_START_GAP_SECONDS: i64 = 12 * 60 * 60;

/// What to do now with one meeting's retained speech.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AudioPlan {
    /// Nothing yet; look again later.
    Wait,
    /// Re-transcribe this meeting on its own. With P2P its speech is kept afterwards, marked as
    /// passed, for a recording that may join it later.
    PassAlone,
    /// Send this meeting's speech to the device that combines the recordings, then delete it here.
    Send {
        /// The combining device.
        to: DeviceId,
    },
    /// Re-transcribe `root` from its own speech and its members', all here, then delete them.
    Combine {
        /// The meeting the recordings were folded into.
        root: MeetingId,
        /// The folded recordings whose speech is here, with their offsets on `root`'s timeline.
        members: Vec<(MeetingId, i64)>,
    },
    /// Delete the speech without a pass.
    Discard,
}

/// How meetings are listed once recordings of one meeting are folded together.
#[derive(Clone, Debug, Default)]
pub struct MeetingListing {
    /// Recordings folded into a meeting that is stored here, which are not listed.
    hidden: HashSet<MeetingId>,
    /// For each listed meeting, its devices: its own first, then its folded recordings'.
    devices: HashMap<MeetingId, Vec<DeviceId>>,
    /// For each listed meeting, the recordings folded into it, oldest first.
    folded: HashMap<MeetingId, Vec<MeetingId>>,
}

impl MeetingListing {
    /// Works out the listing from every meeting's recording facts.
    ///
    /// Inputs: The recordings, as `NoteRepository::meeting_recordings` lists them. Outputs: The
    /// listing. A recording folded into a meeting not stored here is listed on its own.
    /// Side effects: None.
    pub fn of(recordings: &[MeetingRecording]) -> Self {
        let stored: HashSet<&MeetingId> = recordings
            .iter()
            .map(|recording| &recording.meeting_id)
            .collect();
        let mut listing = Self::default();
        for recording in recordings {
            listing
                .devices
                .entry(recording.meeting_id.clone())
                .or_default()
                .insert(0, recording.acquisition_device_id.clone());
            let Some(combination) = recording
                .combined_into
                .as_ref()
                .filter(|combination| stored.contains(&combination.into))
            else {
                continue;
            };
            listing.hidden.insert(recording.meeting_id.clone());
            listing
                .folded
                .entry(combination.into.clone())
                .or_default()
                .push(recording.meeting_id.clone());
            let devices = listing.devices.entry(combination.into.clone()).or_default();
            if !devices.contains(&recording.acquisition_device_id) {
                devices.push(recording.acquisition_device_id.clone());
            }
        }
        listing
    }

    /// Reports whether `meeting_id` is listed: false for a recording folded into another.
    pub fn is_listed(&self, meeting_id: &MeetingId) -> bool {
        !self.hidden.contains(meeting_id)
    }

    /// Returns the devices that recorded `meeting_id`: its own first, then those of the
    /// recordings folded into it. Empty for an unknown meeting.
    pub fn recorded_on(&self, meeting_id: &MeetingId) -> Vec<DeviceId> {
        self.devices.get(meeting_id).cloned().unwrap_or_default()
    }

    /// Returns the recordings folded into `meeting_id`, which Split would list again.
    pub fn folded_into(&self, meeting_id: &MeetingId) -> Vec<MeetingId> {
        self.folded.get(meeting_id).cloned().unwrap_or_default()
    }
}

/// A device that recorded a meeting, as the apps show it ("recorded on Laptop + Phone").
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RecordingDevice {
    /// The ID written on the device's meetings.
    pub device_id: DeviceId,
    /// Its name: this device's own, or the name a paired device gave when pairing; `None` when
    /// the device is not paired here or has not sent its manifest yet.
    pub name: Option<String>,
    /// Whether it is this device.
    pub this_device: bool,
}

/// Names the devices that recorded a meeting from the P2P status.
///
/// Inputs: The devices, as `MeetingListing::recorded_on` lists them, and the P2P status (`None`
/// without P2P, when no device has a name). Outputs: The devices in the same order.
/// Side effects: None.
pub fn name_recording_devices(
    devices: &[DeviceId],
    status: Option<&crate::application::p2p_service::P2pStatus>,
) -> Vec<RecordingDevice> {
    devices
        .iter()
        .map(|device| {
            let this_device = status.is_some_and(|status| {
                status.local_manifest.storage_device_id.as_ref() == Some(device)
            });
            let name = status.and_then(|status| {
                if this_device {
                    return Some(status.device_name.as_str().to_owned());
                }
                let peer = status
                    .peer_manifests
                    .iter()
                    .find(|manifest| manifest.storage_device_id.as_ref() == Some(device))?
                    .device_id();
                status
                    .trusted_devices
                    .iter()
                    .find(|trusted| trusted.peer_id() == peer)
                    .map(|trusted| trusted.device_name().as_str().to_owned())
            });
            RecordingDevice {
                device_id: device.clone(),
                name,
                this_device,
            }
        })
        .collect()
}

/// Splits recordings of one meeting apart again, for good (the Split action).
///
/// Inputs: The repository and either the listed meeting (all recordings folded into it are split)
/// or one folded recording (only it is split).
/// Outputs: The recordings that are listed again; empty when there was nothing to split.
/// Side effects: Writes each split recording's document, which syncs to paired devices. The
/// listed meeting keeps its transcript, which may already combine every device's speech; each
/// split recording shows its own live transcript again.
pub fn split_recordings(
    repository: &dyn NoteRepository,
    meeting_id: &MeetingId,
) -> Result<Vec<MeetingId>, StorageError> {
    let recordings = repository.meeting_recordings()?;
    let folded_here = recordings
        .iter()
        .any(|recording| recording.meeting_id == *meeting_id && recording.combined_into.is_some());
    let to_split: Vec<MeetingId> = if folded_here {
        vec![meeting_id.clone()]
    } else {
        MeetingListing::of(&recordings).folded_into(meeting_id)
    };
    let mut split = Vec::with_capacity(to_split.len());
    for recording in to_split {
        if repository.split_meeting(&recording)? {
            split.push(recording);
        }
    }
    Ok(split)
}

/// Speech of a meeting held on this device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeldSpeech {
    /// How long ago it was last written here.
    pub age: Duration,
    /// Whether the meeting already took the pass alone with it.
    pub passed_alone: bool,
}

/// What `plan_retained_audio` knows about this device.
pub struct PlanContext<'context> {
    /// This device, by the ID written on its meetings.
    pub this_device: &'context DeviceId,
    /// Whether it can run the pass.
    pub can_pass: bool,
    /// Whether another device's recording can still reach it (P2P is on), so speech is worth
    /// keeping after a pass alone.
    pub keeps_for_partners: bool,
    /// The speech held here for a meeting, if any.
    pub held: &'context dyn Fn(&MeetingId) -> Option<HeldSpeech>,
}

/// The recordings of one meeting: the first, and those folded into it with their offsets.
struct Group<'recordings> {
    root: &'recordings MeetingRecording,
    members: Vec<(&'recordings MeetingRecording, i64)>,
}

impl<'recordings> Group<'recordings> {
    /// Collects the group whose first recording is `root`; `None` when it is not stored here.
    fn of(root: &MeetingId, recordings: &'recordings [MeetingRecording]) -> Option<Self> {
        let root = recordings
            .iter()
            .find(|recording| recording.meeting_id == *root)?;
        let members = recordings
            .iter()
            .filter_map(|recording| {
                recording
                    .combined_into
                    .as_ref()
                    .filter(|combination| combination.into == root.meeting_id)
                    .map(|combination| (recording, combination.offset_ms))
            })
            .collect();
        Some(Self { root, members })
    }

    /// Every recording, the first one first.
    fn recordings(&self) -> impl Iterator<Item = &'recordings MeetingRecording> + '_ {
        std::iter::once(self.root).chain(self.members.iter().map(|(member, _)| *member))
    }
}

/// Returns the device that combines a meeting's recordings.
///
/// Inputs: The meeting the recordings were folded into, and every meeting as a recording.
/// Outputs: The first recording's device that is not a phone, by start time, or the first
/// recording's device when all are phones. When some recording does not say whether a phone made
/// it, the device the first fold named. `None` when nothing is folded into the meeting.
/// Side effects: None.
pub fn combining_device(root: &MeetingId, recordings: &[MeetingRecording]) -> Option<DeviceId> {
    let group = Group::of(root, recordings)?;
    let (_, _) = group.members.first()?;
    combiner_among(&group.recordings().collect::<Vec<_>>()).or_else(|| {
        group
            .members
            .first()
            .and_then(|(member, _)| member.combined_into.as_ref())
            .map(|combination| combination.combine_on.clone())
    })
}

/// The combining device of these recordings by device kind; `None` when a kind is unknown.
fn combiner_among(recordings: &[&MeetingRecording]) -> Option<DeviceId> {
    let mut by_start: Vec<&MeetingRecording> = recordings.to_vec();
    if by_start
        .iter()
        .any(|recording| recording.recorded_on_phone.is_none())
    {
        return None;
    }
    by_start.sort_by_key(|recording| recording.created_at);
    by_start
        .iter()
        .find(|recording| recording.recorded_on_phone == Some(false))
        .or_else(|| by_start.first())
        .map(|recording| recording.acquisition_device_id.clone())
}

/// Finds recordings of one meeting made on several devices, at least one of each pair this one.
///
/// Inputs: Every meeting as a recording (oldest first, as `NoteRepository::meeting_recordings`
/// lists them); a loader of a meeting's segments; this device; whether it is a phone; and the
/// oldest start considered.
/// Outputs: For each recording found to belong to another's meeting, the recording and how to
/// fold it: into the first recording of that meeting, with its offset on that recording's
/// timeline. Only a recording that stands alone is folded; two meetings that each already have
/// folded recordings are left apart. Recordings split from any recording of a meeting, or of one
/// device, are not joined. A loader error is returned.
/// Side effects: Only the loader's reads.
pub fn find_combinations(
    recordings: &[MeetingRecording],
    load_segments: &dyn Fn(&MeetingId) -> Result<Vec<TranscriptSegment>, StorageError>,
    this_device: &DeviceId,
    this_device_is_phone: bool,
    since: DateTime<Utc>,
) -> Result<Vec<(MeetingId, MeetingCombination)>, StorageError> {
    // Where each recording sits: the first recording of its meeting and its offset there.
    let mut placement: HashMap<MeetingId, (MeetingId, i64)> = HashMap::new();
    for recording in recordings {
        let place = match &recording.combined_into {
            Some(combination) => (combination.into.clone(), combination.offset_ms),
            None => (recording.meeting_id.clone(), 0),
        };
        placement.insert(recording.meeting_id.clone(), place);
    }
    let mut has_members: HashSet<MeetingId> = placement
        .iter()
        .filter(|(meeting, (root, _))| meeting != &root)
        .map(|(_, (root, _))| root.clone())
        .collect();
    let stands_alone = |meeting: &MeetingId,
                        placement: &HashMap<MeetingId, (MeetingId, i64)>,
                        has_members: &HashSet<MeetingId>| {
        placement
            .get(meeting)
            .is_none_or(|(root, _)| root == meeting)
            && !has_members.contains(meeting)
    };
    let candidates: Vec<&MeetingRecording> = recordings
        .iter()
        .filter(|recording| recording.created_at >= since)
        .collect();
    let mut texts: HashMap<MeetingId, RecordingText> = HashMap::new();
    let mut found = Vec::new();
    for first in 0..candidates.len() {
        for second in first + 1..candidates.len() {
            let (earlier, later) = (candidates[first], candidates[second]);
            if !may_pair(earlier, later, this_device) {
                continue;
            }
            let (Some(earlier_place), Some(later_place)) = (
                placement.get(&earlier.meeting_id).cloned(),
                placement.get(&later.meeting_id).cloned(),
            ) else {
                continue;
            };
            if earlier_place.0 == later_place.0 {
                continue;
            }
            // The recording that joins stands alone; it joins the other's meeting.
            let (joining, other, other_place) =
                if stands_alone(&later.meeting_id, &placement, &has_members) {
                    (later, earlier, earlier_place)
                } else if stands_alone(&earlier.meeting_id, &placement, &has_members) {
                    (earlier, later, later_place)
                } else {
                    continue;
                };
            let (root, other_offset) = other_place;
            let group: Vec<&MeetingRecording> = recordings
                .iter()
                .filter(|recording| {
                    placement
                        .get(&recording.meeting_id)
                        .is_some_and(|(recording_root, _)| *recording_root == root)
                })
                .collect();
            let kept_apart = group.iter().any(|recording| {
                recording.kept_apart_from.contains(&joining.meeting_id)
                    || joining.kept_apart_from.contains(&recording.meeting_id)
            });
            if kept_apart {
                continue;
            }
            for recording in [joining, other] {
                if !texts.contains_key(&recording.meeting_id) {
                    let text = RecordingText::from_segments(
                        recording.created_at.timestamp_millis(),
                        &load_segments(&recording.meeting_id)?,
                    );
                    texts.insert(recording.meeting_id.clone(), text);
                }
            }
            let (Some(other_text), Some(joining_text)) =
                (texts.get(&other.meeting_id), texts.get(&joining.meeting_id))
            else {
                continue;
            };
            let Some(same) = recognise_same_meeting(other_text, joining_text) else {
                continue;
            };
            let mut joined: Vec<&MeetingRecording> = group;
            joined.push(joining);
            let combine_on = combiner_among(&joined).unwrap_or_else(|| {
                if this_device_is_phone {
                    if joining.acquisition_device_id == *this_device {
                        other.acquisition_device_id.clone()
                    } else {
                        joining.acquisition_device_id.clone()
                    }
                } else {
                    this_device.clone()
                }
            });
            let offset_ms = same.offset_ms() + other_offset;
            placement.insert(joining.meeting_id.clone(), (root.clone(), offset_ms));
            has_members.insert(root.clone());
            found.push((
                joining.meeting_id.clone(),
                MeetingCombination {
                    into: root,
                    offset_ms,
                    combine_on,
                },
            ));
        }
    }
    Ok(found)
}

/// Whether two recordings may be compared: one recorded here, the other on another device,
/// started within `MAX_START_GAP_SECONDS`, and never split apart by the owner.
fn may_pair(earlier: &MeetingRecording, later: &MeetingRecording, this_device: &DeviceId) -> bool {
    let recorded_here = earlier.acquisition_device_id == *this_device
        || later.acquisition_device_id == *this_device;
    let kept_apart = earlier.kept_apart_from.contains(&later.meeting_id)
        || later.kept_apart_from.contains(&earlier.meeting_id);
    recorded_here
        && earlier.acquisition_device_id != later.acquisition_device_id
        && (later.created_at - earlier.created_at).num_seconds().abs() <= MAX_START_GAP_SECONDS
        && !kept_apart
}

/// Decides what to do with one meeting's retained speech.
///
/// Inputs: The meeting, held here; every meeting as a recording; and what this device holds and
/// can do.
/// Outputs: The plan (see the module header). A meeting unknown to the list is planned alone.
/// Side effects: None.
pub fn plan_retained_audio(
    meeting_id: &MeetingId,
    recordings: &[MeetingRecording],
    context: &PlanContext<'_>,
) -> AudioPlan {
    let held = context.held;
    let Some(own) = held(meeting_id) else {
        return AudioPlan::Wait;
    };
    let root_id = recordings
        .iter()
        .find(|recording| recording.meeting_id == *meeting_id)
        .and_then(|recording| recording.combined_into.as_ref())
        .map_or_else(
            || meeting_id.clone(),
            |combination| combination.into.clone(),
        );
    let group = Group::of(&root_id, recordings).filter(|group| !group.members.is_empty());
    let Some(group) = group else {
        return plan_alone(own, context);
    };
    let Some(combiner) = combining_device(&root_id, recordings) else {
        return plan_alone(own, context);
    };
    if combiner != *context.this_device {
        return if own.age < KEEP_FOR_COMBINING {
            AudioPlan::Send { to: combiner }
        } else {
            AudioPlan::Discard
        };
    }
    let Some(root_speech) = held(&root_id) else {
        // The first recording's speech is gone if it was recorded here (it was combined or
        // expired); otherwise its device still has to send it.
        let root_recorded_here = group.root.acquisition_device_id == *context.this_device;
        return if root_recorded_here || own.age >= KEEP_FOR_COMBINING {
            AudioPlan::Discard
        } else {
            AudioPlan::Wait
        };
    };
    let held_members: Vec<(MeetingId, i64)> = group
        .members
        .iter()
        .filter(|(member, _)| held(&member.meeting_id).is_some())
        .map(|(member, offset_ms)| (member.meeting_id.clone(), *offset_ms))
        .collect();
    let combine_now = AudioPlan::Combine {
        root: root_id.clone(),
        members: held_members.clone(),
    };
    if held_members.len() == group.members.len() {
        return if context.can_pass {
            combine_now
        } else if root_speech.age < KEEP_FOR_COMBINING {
            AudioPlan::Wait
        } else {
            AudioPlan::Discard
        };
    }
    if root_speech.age < COMBINING_GRACE {
        return AudioPlan::Wait;
    }
    if root_speech.age < KEEP_FOR_COMBINING {
        // The first recording takes its pass alone while the others are awaited; a received
        // recording leaves that to the first recording's own plan.
        let pass_alone_now =
            !root_speech.passed_alone && context.can_pass && *meeting_id == root_id;
        return if pass_alone_now {
            AudioPlan::PassAlone
        } else {
            AudioPlan::Wait
        };
    }
    if context.can_pass && (!held_members.is_empty() || !root_speech.passed_alone) {
        combine_now
    } else {
        AudioPlan::Discard
    }
}

/// Plans a meeting no other recording has joined.
fn plan_alone(own: HeldSpeech, context: &PlanContext<'_>) -> AudioPlan {
    if own.age < COMBINING_GRACE {
        AudioPlan::Wait
    } else if !own.passed_alone && context.can_pass {
        AudioPlan::PassAlone
    } else if (context.keeps_for_partners || !context.can_pass) && own.age < KEEP_FOR_COMBINING {
        AudioPlan::Wait
    } else {
        AudioPlan::Discard
    }
}
