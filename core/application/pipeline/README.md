<!-- Explains the end-to-end meeting pipeline that turns audio chunks into speaker-attributed segments. -->

# Meeting pipeline

This directory connects transcription, diarization, voice identity, and storage into one
application-layer flow. It uses only existing ports: `TranscriptionEngine`, `DiarizationEngine`,
`VoiceIdentityMatcher`, `IdentityMapper`, and `NoteRepository`. No adapter, model, or SQL detail
is imported.

## Contents

| File | Purpose |
| --- | --- |
| `meeting_session.rs` | `MeetingSession`: per-recording orchestration of local VAD chunks and external bridge chunks. |
| `alignment.rs` | Pure timestamp alignment and transcript text distribution against VAD chunk boundaries. |
| `pipeline_factory.rs` | `PipelineFactory`: the composition root that owns the pipeline ports once and opens one session per recording. |
| `tests.rs` | Mock-port TDD specification plus an in-memory SQLite integration test of a three-chunk meeting. |
| `pipeline_factory_tests.rs` | Factory availability, graceful degradation, meeting creation, engine reuse, and clustering policy. |

## MeetingSession

`MeetingSession::start` is called when a recording starts for an existing meeting. It holds the
meeting ID, the transcription engine chosen by compute routing, an optional diarization engine, the
voice-identity matcher, and the note repository. It loads the meeting's existing segments, because
`save_transcript_segments` replaces a meeting's whole timeline. Later chunks are appended to that
timeline instead of overwriting it. The previous chunk's text is passed as `TranscriptionContext`.

## PipelineFactory (Issue #32)

Entry points build one `PipelineFactory` at start-up and call `create_session(&meeting_id)` each
time a recording starts. The factory owns the transcription engine, an optional diarization
engine, an optional voice matcher, the note repository, and the clustering policy. Sessions
borrow these through the ports' `&T` forwarding impls, so engines and loaded models are reused
across recordings. `create_session` creates the meeting (an empty Automerge document) when it
does not exist and otherwise appends to its existing timeline.

Missing dependencies degrade instead of failing: without a diarization engine
(`NoDiarizationEngine` is the placeholder type) segments are `Unattributed`; without a matcher,
`OptionalVoiceMatcher` reports `NoMatch`, so diarized voices are `Unknown` and clustered.
`availability()` reports both. The transcription engine and repository are required, so an entry
point that has no engine fails at composition (see `core/bin/pipeline_composition.rs`).

`application::recording_manager::RecordingManager` runs these sessions on a worker thread for
live recordings; see `../README.md`.

## Local VAD chunk flow

```text
VadAudioChunk (recording, meeting-relative start/end, opaque audio)
  │
  ├─1─ TranscriptionEngine::transcribe ──── error → log, return Err, persist nothing
  ├─2─ DiarizationEngine::diarize ───────── missing/error → whole chunk "Unattributed"
  ├─3─ VoiceIdentityMatcher per segment
  │      strong   → Known(identity)
  │      weak     → "Unknown (pending review)" (with candidate) ┐
  │      none     → "Unknown"                                   ├─ UnknownSpeakerClusterer
  │      error    → "Unknown", log, continue                    ┘
  │             founds a cluster  → UnassignedVoiceClip linked to the cluster
  │             joins a cluster   → no new clip (the cluster's clip represents it)
  │             clustering error  → unclustered UnassignedVoiceClip, log, continue
  ├─4─ alignment::align_chunk_regions (merge text with speaker regions)
  └─5─ NoteRepository::save_transcript_segments (whole meeting timeline)
```

## External bridge (Teams) flow

`process_external_chunk` never calls diarization: the bridge guarantees one isolated speaker per
stream. It transcribes through `transcribe_external_media` and then resolves the participant through
`IdentityMapper`. It persists one `Known` segment from `timestamp_ms` to
`timestamp_ms + duration_ms`. A mapping failure degrades the segment to `Unknown`, with no review
clip because there is no embedding to review. Voice auto-enrollment stays in `TeamsProcessor`.

## Timestamp alignment

The transcription port returns chunk-level text without word timestamps, so the VAD chunk is the
alignment reference:

1. Diarization regions, which are relative to the chunk, are clamped to the chunk duration and
   sorted. Overlaps are cut at the earlier region's end.
2. Adjacent regions with the same attribution are merged. A sentence that transcription splits
   but diarization keeps together therefore stays one segment.
3. Regions are stretched to tile the chunk. The first region starts at 0, the last ends at the
   chunk end, and each gap is split at its midpoint.
4. Words are distributed in proportion to each region's duration, using rounded cumulative
   boundaries. Regions that receive no words are dropped. Blank transcripts produce no segment.
5. Offsets are shifted by the chunk start into meeting time.

Without usable regions, the whole chunk becomes one `Unattributed` segment. Proportional word
distribution is a heuristic. Word-level timestamps from the transcription port would replace it.

## Unknown-speaker clusters

Weak and unmatched voices are clustered per meeting by cosine similarity (threshold 0.70, see
`../README.md`). `SpeakerAttribution::{PendingReview, Unknown}` carry the `cluster_id`, which is
persisted as `TranscriptSegment::unknown_cluster_id`. Only the chunk that founds a cluster saves a
review clip, so the Voice Gallery shows one clip per unknown person rather than one per chunk.
`MeetingSession::assign_identity_to_cluster` enrolls the cluster's embeddings, relinks its
segments, resolves the cluster, and reloads the session's in-memory timeline. Assign through the
session while it is live; otherwise its next chunk would rewrite the stale timeline.

## Persistence and labels

`TranscriptSegment` stores only `Option<Uuid>`, so only `Known` segments carry an identity.
"Unknown (pending review)", "Unknown", and "Unattributed" are all persisted as `None`, and their
distinction is returned in `AttributedSegment::attribution()`. Review clips keep chunk audio and
derived embeddings in `NoteRepository` until a user resolves them in the Voice Gallery. Logs carry
meeting IDs, offsets, counts, and clip IDs only, never text, audio, or embeddings.

## Runtime note

`TranscriptionEngine` is synchronous. Drive a session from a blocking-capable worker, such as a
dedicated thread or `spawn_blocking`, rather than directly on the main async runtime.
