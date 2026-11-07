//! Folds devices' recordings of one meeting together and decides what each device does with its
//! retained speech afterwards (#88).
//!
//! The owner may record a meeting on the laptop and on a phone placed near the other person. Both
//! recordings sync as ordinary meetings; `find_combinations` recognises them
//! (`same_meeting::recognise_same_meeting`) and folds the later recording into the earlier one.
//! Only a device that recorded one of the two decides, so a third paired device never does. The
//! fold names the device that combines their audio: a desktop that finds the pair names itself,
//! and a phone names the other device. Two devices that find the pair at once may choose
//! differently; the meeting document converges on one choice, and until then they only wait.
//!
//! `plan_retained_audio` then says what to do with one meeting's sealed speech (#87):
//! - a meeting alone waits `COMBINING_GRACE` for a partner to appear, then takes the pass alone;
//!   a device without the pass keeps it at most `KEEP_FOR_COMBINING`, then deletes it;
//! - a device that does not combine sends its recording to the one that does, for at most
//!   `KEEP_FOR_COMBINING`;
//! - the combining device waits until every recording of the meeting is here, at most
//!   `KEEP_FOR_COMBINING`, then combines what it has.
//!
//! `MeetingListing` says how folded meetings are shown: once, under the first recording, as
//! recorded on every device that took part.
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

/// How long a finished recording waits for another device's recording of it to sync here.
pub const COMBINING_GRACE: Duration = Duration::from_secs(10 * 60);
/// Longest a recording's speech is kept waiting to be combined: on a device without the pass, or
/// on the combining device for a recording that has not arrived.
pub const KEEP_FOR_COMBINING: Duration = Duration::from_secs(24 * 60 * 60);
/// Recordings started further apart than this are never compared.
const MAX_START_GAP_SECONDS: i64 = 12 * 60 * 60;

/// What to do now with one meeting's retained speech.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AudioPlan {
    /// Nothing yet; look again later.
    Wait,
    /// Re-transcribe this meeting on its own.
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

/// Finds recordings of one meeting made on two devices, at least one of them this one.
///
/// Inputs: Every meeting as a recording (oldest first, as `NoteRepository::meeting_recordings`
/// lists them); a loader of a meeting's segments; this device; whether it is a phone; and the
/// oldest start considered.
/// Outputs: For each pair found, the later meeting and how to fold it into the earlier; recordings
/// already folded, split from each other, or from one device are skipped. A loader error is
/// returned.
/// Side effects: Only the loader's reads.
pub fn find_combinations(
    recordings: &[MeetingRecording],
    load_segments: &dyn Fn(&MeetingId) -> Result<Vec<TranscriptSegment>, StorageError>,
    this_device: &DeviceId,
    this_device_is_phone: bool,
    since: DateTime<Utc>,
) -> Result<Vec<(MeetingId, MeetingCombination)>, StorageError> {
    let candidates: Vec<&MeetingRecording> = recordings
        .iter()
        .filter(|recording| recording.created_at >= since && recording.combined_into.is_none())
        .collect();
    let mut texts: Vec<Option<RecordingText>> = vec![None; candidates.len()];
    let mut folded: Vec<bool> = vec![false; candidates.len()];
    let mut found = Vec::new();
    for first in 0..candidates.len() {
        for second in first + 1..candidates.len() {
            let (earlier, later) = (candidates[first], candidates[second]);
            if folded[first] || folded[second] || !may_pair(earlier, later, this_device) {
                continue;
            }
            for index in [first, second] {
                if texts[index].is_none() {
                    let recording = candidates[index];
                    texts[index] = Some(RecordingText::from_segments(
                        recording.created_at.timestamp_millis(),
                        &load_segments(&recording.meeting_id)?,
                    ));
                }
            }
            let (Some(earlier_text), Some(later_text)) = (&texts[first], &texts[second]) else {
                continue;
            };
            let Some(same) = recognise_same_meeting(earlier_text, later_text) else {
                continue;
            };
            let other_device = if earlier.acquisition_device_id == *this_device {
                &later.acquisition_device_id
            } else {
                &earlier.acquisition_device_id
            };
            let combine_on = if this_device_is_phone {
                other_device.clone()
            } else {
                this_device.clone()
            };
            folded[second] = true;
            found.push((
                later.meeting_id.clone(),
                MeetingCombination {
                    into: earlier.meeting_id.clone(),
                    offset_ms: same.offset_ms(),
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
/// Inputs: The meeting; every meeting as a recording; whether a meeting's speech is held here;
/// this device; how long ago the meeting's speech was last written here; and whether this device
/// can run the pass.
/// Outputs: The plan (see the module header). A meeting unknown to the list is planned alone.
/// Side effects: None.
pub fn plan_retained_audio(
    meeting_id: &MeetingId,
    recordings: &[MeetingRecording],
    held: &dyn Fn(&MeetingId) -> bool,
    this_device: &DeviceId,
    age: Duration,
    can_pass: bool,
) -> AudioPlan {
    let find = |id: &MeetingId| {
        recordings
            .iter()
            .find(|recording| &recording.meeting_id == id)
    };
    let send_or_discard = |to: &DeviceId| {
        if age < KEEP_FOR_COMBINING {
            AudioPlan::Send { to: to.clone() }
        } else {
            AudioPlan::Discard
        }
    };
    let root = match find(meeting_id).and_then(|recording| recording.combined_into.as_ref()) {
        Some(combination) if combination.combine_on != *this_device => {
            return send_or_discard(&combination.combine_on);
        }
        Some(combination) => combination.into.clone(),
        None => meeting_id.clone(),
    };
    let members: Vec<(&MeetingId, &MeetingCombination)> = recordings
        .iter()
        .filter_map(|recording| {
            recording
                .combined_into
                .as_ref()
                .filter(|combination| combination.into == root)
                .map(|combination| (&recording.meeting_id, combination))
        })
        .collect();
    let Some((_, first_member)) = members.first() else {
        return if age < COMBINING_GRACE {
            AudioPlan::Wait
        } else if can_pass {
            AudioPlan::PassAlone
        } else if age < KEEP_FOR_COMBINING {
            AudioPlan::Wait
        } else {
            AudioPlan::Discard
        };
    };
    if first_member.combine_on != *this_device {
        return send_or_discard(&first_member.combine_on);
    }
    if !held(&root) {
        let root_recorded_here =
            find(&root).is_some_and(|recording| recording.acquisition_device_id == *this_device);
        return if root_recorded_here || age >= KEEP_FOR_COMBINING {
            AudioPlan::Discard
        } else {
            AudioPlan::Wait
        };
    }
    let all_here = members.iter().all(|(member, _)| held(member));
    if !all_here && age < KEEP_FOR_COMBINING {
        return AudioPlan::Wait;
    }
    if !can_pass {
        return if age < KEEP_FOR_COMBINING {
            AudioPlan::Wait
        } else {
            AudioPlan::Discard
        };
    }
    AudioPlan::Combine {
        root,
        members: members
            .into_iter()
            .filter(|(member, _)| held(member))
            .map(|(member, combination)| (member.clone(), combination.offset_ms))
            .collect(),
    }
}
