//! Specifies the `NTC1` messages that copy an unknown-speaker cluster, its member embeddings, and
//! its review clips to a paired device (Issue #50): exact round trips under the pair key, clips
//! split across messages to stay within the audio budget, and rejection of messages that are
//! tampered with, sent the other way, or implausible.

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::cluster_share_exchange::{
    ClusterShareMessageError, decode_cluster_share_message, encode_cluster_share_messages,
    is_cluster_share_message,
};
use crate::domain::{
    models::{
        ClusterShare, DeviceId, MeetingId, UnassignedVoiceClip, UnknownSpeakerChunk,
        UnknownSpeakerCluster, WeakVoiceMatchCandidate,
    },
    peer_trust::PairKey,
};

/// The cluster used throughout.
fn cluster_id() -> Uuid {
    Uuid::from_u128(0xC1)
}

/// Builds a share with two member chunks and `clip_audio_lengths.len()` clips.
fn share(clip_audio_lengths: &[usize]) -> ClusterShare {
    let meeting_id = MeetingId::new("meeting-1");
    let chunks = vec![
        UnknownSpeakerChunk::new("chunk-a", vec![0.25, -0.5, 1.0], 1_200),
        UnknownSpeakerChunk::new("chunk-b", vec![0.125, 0.5, -1.0], 800),
    ];
    let cluster = UnknownSpeakerCluster::new(
        cluster_id(),
        meeting_id.clone(),
        vec![0.1875, 0.0, 0.0],
        vec![String::from("chunk-a"), String::from("chunk-b")],
        2_000,
        Utc.timestamp_millis_opt(1_700_000_000_123)
            .single()
            .unwrap_or_else(|| panic!("valid timestamp")),
        None,
    );
    let clips = clip_audio_lengths
        .iter()
        .enumerate()
        .map(|(index, length)| {
            let offset = u64::try_from(index).unwrap_or_default() * 1_000;
            UnassignedVoiceClip::with_review_context(
                Uuid::from_u128(0x100 + index as u128),
                meeting_id.clone(),
                offset,
                offset + 900,
                vec![u8::try_from(index % 250).unwrap_or_default(); *length],
                vec![0.5, 0.25, -0.125],
                (index == 0).then(|| WeakVoiceMatchCandidate::new(Uuid::from_u128(0xA1), 0.42)),
            )
            .with_cluster_id(Some(cluster_id()))
        })
        .collect();
    ClusterShare::new(cluster, chunks, clips)
}

/// The two devices of the pair.
fn devices() -> (DeviceId, DeviceId) {
    (DeviceId::new("peer-laptop"), DeviceId::new("peer-phone"))
}

/// Encodes `share` from the laptop to the phone with `audio_budget` bytes of audio per message.
fn encode(share: &ClusterShare, audio_budget: usize) -> Vec<Vec<u8>> {
    let (laptop_id, phone_id) = devices();
    encode_cluster_share_messages(
        share,
        &PairKey::from_bytes([6; 32]),
        &laptop_id,
        &phone_id,
        audio_budget,
    )
    .unwrap_or_else(|error| panic!("share must encode: {error}"))
}

/// Decodes a message the phone received from the laptop.
fn decode(message: &[u8]) -> Result<ClusterShare, ClusterShareMessageError> {
    let (laptop_id, phone_id) = devices();
    decode_cluster_share_message(
        message,
        &PairKey::from_bytes([6; 32]),
        &laptop_id,
        &phone_id,
    )
}

/// A share fitting the budget travels in one message and arrives unchanged, while neither the
/// audio nor the embeddings appear in the bytes on the wire.
#[test]
fn a_share_round_trips_in_one_sealed_message() {
    let original = share(&[64, 32]);

    let messages = encode(&original, 1_024);

    assert_eq!(messages.len(), 1);
    assert!(is_cluster_share_message(&messages[0]));
    assert!(!messages[0].windows(32).any(|window| window == [0_u8; 32]));
    assert!(
        !messages[0]
            .windows(4)
            .any(|window| window == 0.25_f32.to_le_bytes())
    );
    assert_eq!(decode(&messages[0]), Ok(original));
}

/// Clips beyond the audio budget go in further messages; each carries the cluster and all its
/// chunks, so every message can be imported alone.
#[test]
fn clips_beyond_the_budget_are_split_across_messages() {
    let original = share(&[600, 600, 300]);

    let messages = encode(&original, 1_000);
    let decoded: Vec<ClusterShare> = messages
        .iter()
        .map(|message| decode(message).unwrap_or_else(|error| panic!("must decode: {error}")))
        .collect();

    assert_eq!(messages.len(), 2);
    for part in &decoded {
        assert_eq!(part.cluster(), original.cluster());
        assert_eq!(part.chunks(), original.chunks());
    }
    let clip_ids: Vec<Uuid> = decoded
        .iter()
        .flat_map(|part| part.clips().iter().map(UnassignedVoiceClip::clip_id))
        .collect();
    let original_ids: Vec<Uuid> = original
        .clips()
        .iter()
        .map(UnassignedVoiceClip::clip_id)
        .collect();
    assert_eq!(clip_ids, original_ids);
}

/// A share without clips still travels, so another device can list and assign the cluster.
#[test]
fn a_share_without_clips_is_one_message() {
    let messages = encode(&share(&[]), 1_000);

    assert_eq!(messages.len(), 1);
    assert_eq!(decode(&messages[0]), Ok(share(&[])));
}

/// A clip larger than the whole budget is left out rather than failing the share.
#[test]
fn a_clip_larger_than_the_budget_is_left_out() {
    let messages = encode(&share(&[2_000, 10]), 1_000);
    let decoded = decode(&messages[0]).unwrap_or_else(|error| panic!("must decode: {error}"));

    assert_eq!(messages.len(), 1);
    assert_eq!(decoded.clips().len(), 1);
    assert_eq!(decoded.clips()[0].audio_bytes().len(), 10);
}

/// A tampered message, or one sent the other way, does not open.
#[test]
fn tampered_or_reflected_messages_are_rejected() {
    let mut message = encode(&share(&[16]), 1_000).remove(0);
    let (laptop_id, phone_id) = devices();
    let reflected = decode_cluster_share_message(
        &message,
        &PairKey::from_bytes([6; 32]),
        &phone_id,
        &laptop_id,
    );
    if let Some(byte) = message.get_mut(40) {
        *byte ^= 1;
    }

    assert_eq!(reflected, Err(ClusterShareMessageError::Unauthenticated));
    assert_eq!(
        decode(&message),
        Err(ClusterShareMessageError::Unauthenticated)
    );
    assert!(!is_cluster_share_message(b"NTS1{}"));
}

/// A sender that holds the pair key is still peer input: implausible values are refused.
#[test]
fn implausible_shares_are_rejected() {
    let (laptop_id, phone_id) = devices();
    let pair_key = PairKey::from_bytes([6; 32]);
    let non_finite = ClusterShare::new(
        UnknownSpeakerCluster::new(
            cluster_id(),
            MeetingId::new("meeting-1"),
            vec![f32::NAN],
            Vec::new(),
            0,
            Utc::now(),
            None,
        ),
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(
        encode_cluster_share_messages(&non_finite, &pair_key, &laptop_id, &phone_id, 1_000),
        Err(ClusterShareMessageError::Invalid)
    );
}
