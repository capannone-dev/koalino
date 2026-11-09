<!-- Explains the end-to-end meeting pipeline that turns audio chunks into speaker-attributed segments. -->

# Meeting pipeline

This directory connects transcription, diarization, voice identity, and storage into one
application-layer flow. It uses only existing ports: `TranscriptionEngine`, `DiarizationEngine`,
`VoiceIdentityMatcher`, `IdentityMapper`, and `NoteRepository`. No adapter, model, or SQL detail
is imported.

## Contents

| File | Purpose |
| --- | --- |
| `meeting_session/` | Parts of `MeetingSession` in their own files: `participant_tracks.rs` credits an online-meeting participant's track by the voices it carries (#103). |
| `external_track_tests.rs` | Participant tracks (#103): one voice goes to the participant, a room's voices are matched one by one, people taking turns in a room are told apart, and a voice is offered once after 30 s unless it sounds like someone else. |
| `meeting_session.rs` | `MeetingSession`: per-recording orchestration of local VAD chunks and external bridge chunks. Each chunk's overlap spans (from `diarize_with_overlap`) are kept in meeting time and recorded with the meeting. Live and final segments carry their confidence marks: overlap, a guessed speaker (a weak match, or no voice nearby), and unclear words (#90, #91). |
| `alignment.rs` | Pure timestamp alignment and transcript text distribution against VAD chunk boundaries; each aligned region lists its unclear words. |
| `pipeline_factory.rs` | `PipelineFactory`: the composition root that owns the pipeline ports once and opens one session per recording. Its `GlossarySource` is read when each session opens, so a saved glossary applies from the next recording without rebuilding anything; the session prompts every chunk with it and drops its echo. |
| `tests.rs` | Mock-port TDD specification plus an in-memory SQLite integration test of a three-chunk meeting. |
| `pipeline_factory_tests.rs` | Factory availability, graceful degradation, meeting creation, engine reuse, and clustering policy. |
| `task_metrics.rs` | `TaskMetricsRecorder` and `TaskProfile`: one telemetry row per completed transcription or diarization. |
| `task_metrics_tests.rs` | Per-chunk telemetry: one row per task, none for failures, and a failing store never fails a chunk. |

## MeetingSession

`MeetingSession::start` is called when a recording starts for an existing meeting. It holds the
meeting ID, the transcription engine chosen by compute routing, an optional diarization engine, the
voice-identity matcher, and the note repository. It loads the meeting's existing segments, because
`save_transcript_segments` replaces a meeting's whole timeline. Later chunks are appended to that
timeline instead of overwriting it. Each chunk is transcribed with an empty `TranscriptionContext`:
passing the previous chunk's text as Whisper's prompt made it repeat or complete that chunk's last
words (Issue #51; measured on jfk.wav and an Italian clip).

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

## Telemetry (Issue #53)

`PipelineFactory::with_task_metrics(recorder)` makes every session save one `TaskMetrics` row
through the `MetricsRepository` port after each successful transcription, and one after each
successful diarization. A row holds the device, task type, model label, duration (also the latency,
since a chunk is processed as soon as it arrives), an energy estimate (the profile's milliwatts
times the duration) and the completion time. It never holds audio, text or embeddings. Failed
tasks record nothing. A save that fails is logged and the chunk continues. The recorder is shared
through `Rc` because the factory and its sessions live on one worker thread. In production,
`core/bin/pipeline_composition.rs` gives it its own SQLite connection and labels local models by
file stem, never by path.

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

A bridge stream is one participant's track, and usually one person speaks on it. In a hybrid
meeting, though, a meeting room or a shared laptop joins as one participant with several people
behind it (#103). `process_external_chunk` transcribes through `transcribe_external_media`,
resolves the participant through `IdentityMapper`, and then diarizes the chunk
(`meeting_session/participant_tracks.rs`, policy in `domain/participant_track_voices.rs`):
- While the track carries one voice, the chunk goes to the participant as one `Known` region from
  `timestamp_ms` to `timestamp_ms + duration_ms`, as before. The same happens without a
  diarization engine or when diarization fails.
- Once the track carries several voices, either in one chunk (no voice holds 90% of the speech and
  the others have 2 s or more) or as a clearly different voice in a later chunk, every later chunk
  is matched voice by voice against the gallery, like local audio. A room's people get their names,
  or join unknown-speaker clusters with review clips. Nothing more is credited to the room. Chunks
  credited before the second voice appeared stay credited.
- After 30 s of clean speech of the one voice, the voice is offered once per meeting through
  `IdentityMapper::keep_participant_voice`, unless the gallery matches it to someone else. The
  bridge's `TeamsIdentityMapper` keeps it only when the owner turned on *Keep meeting
  participants' voices* (off by default; voice prints are biometric). A track with several voices
  is never offered.

A mapping failure degrades the participant's part to `Unknown`, with no review clip.
`TeamsProcessor` (in `application/`) still stores an embedding per chunk without these checks; it
is not wired into the bridge and must not be.

## Timestamp alignment

The transcription port returns chunk-level text, with word timing when the engine has it. The VAD chunk is the
alignment reference:

1. Diarization regions, which are relative to the chunk, are clamped to the chunk duration and
   sorted. Overlaps are cut at the earlier region's end.
2. Adjacent regions with the same attribution are merged. A sentence that transcription splits
   but diarization keeps together therefore stays one segment.
3. Regions are stretched to tile the chunk. The first region starts at 0, the last ends at the
   chunk end, and each gap is split at its midpoint.
4. With word timing (local whisper.cpp), each word goes to the region at its midpoint and never back
   to an earlier one. Without it (OpenAI, remote hosts), words are distributed in proportion to each
   region's duration, using rounded cumulative boundaries. Regions that receive no words are dropped.
   Blank transcripts produce no segment.
5. Offsets are shifted by the chunk start into meeting time.

Without usable regions, the whole chunk becomes one `Unattributed` segment. On the evaluation
recordings with known speaker times, word timing puts 98.2% of words right and proportional
distribution 91.5% (#69).

## Unknown-speaker clusters

Weak and unmatched voices are clustered per meeting by cosine similarity (threshold 0.40, see
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

## Final pass (#73, #74)

`meeting_speaker_pass.rs` holds the pure policy, and `MeetingSession::finish_meeting` applies it when a recording stops. It
groups the recording's short voice windows, gives every word the nearest window's speaker, names each speaker once, and
replaces the live segments. Unnamed speakers keep a live cluster, now holding one averaged voice. Every other live cluster
of the recording is deleted, including turns that got no words (#82). A speaker needs at least 8 windows, or in a long
recording 1/30 of them up to 30 (#83), or it joins the nearest speaker. Without windows, or after a crash, the live result stands. See `DECISION_LOG.md` for the figures.
