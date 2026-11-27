<!-- This directory contains use-case orchestration that coordinates domain types through ports. -->

# Application layer

Application services implement workflows such as recording processing and compute selection. They contain no direct infrastructure integrations. `compute_router.rs` selects an injected `TranscriptionEngine` port from portable local/remote `DeviceManifest` values; concrete model and encrypted-P2P implementations remain outside the application layer.

`network_events.rs` consumes only `NetworkEvent` values supplied by a `P2pNetwork` port and keeps
an application-visible reachability view. It has no dependency on libp2p identities, swarm events,
or socket APIs.

`meeting_document.rs` (Issue #43) implements `MeetingDocumentCodec`, the schema that makes the
Automerge document the source of truth for everything that syncs:

- **`origin`:** a map, written once.
- **`transcript`:** a string.
- **`segments`:** a map keyed by `<start_ms:012>-<end_ms:012>[-n]`. Each segment's fields are
  separate CRDT registers, so a speaker relink on one device and a text fix on another both
  survive a merge.
- **`cluster_resolutions`** (Issue #50): a map from unknown-speaker cluster ID to the identity
  it was named as. Concurrent namings of one cluster converge on Automerge's winner, and a
  segment of a resolved cluster projects with that identity as its speaker on every device.
- **`transcription_languages`:** a map from language code (`auto` or a Whisper code) to the
  meeting offset where it was first used. `with_transcription_language` keeps the earliest offset,
  so recordings on two devices merge into one list ordered by first use; unknown codes from a peer
  are skipped. `MeetingSession` records each chunk's language once per recording.
- **`with_segments`:** writes only the fields that changed and deletes segments that are gone.
- **`project`:** skips malformed peer entries with a warning, and a field the document lacks
  projects as `None`.

`pairing_manager.rs` (Issue #43) pairs the owner's devices by numeric comparison:

- **Pairable device:** `make_pairable` opens its pairing window.
- **Initiating device:** `start_pairing` returns the shared `PairingCode`. After an
  `IncomingRequest` reaches the other device, the initiator trusts that device only once both of
  these have happened, in either order:
  - its owner calls `confirm_code`;
  - the other device's acceptance arrives.
- **Responding device:** `accept_incoming` trusts the initiator at once and answers;
  `reject_incoming` declines.
- **Every trust decision:** persisted in `TrustedPeerStore` before the transport admits the peer.
- **Ending pairing:** happens only once any answer has been sent.
- **Unsolicited acceptances:** ignored.
- **Other operations:** `load_trusted_peers` restores the allowlist at startup, and `revoke`
  removes a device from both the store and the transport.

`sync_manager.rs` drives the Automerge sync handshake for one meeting document. On discovery it
generates a peer-specific handshake; on a received envelope it validates the protocol, applies the
message through `MeetingCrdtService`, saves the merged document through `NoteRepository`, and sends
the next acknowledgement when required. After a locally persisted edit, `notify_local_change()`
broadcasts a fresh encrypted `SyncMessage`. The manager uses only `P2pNetwork` and
`NoteRepository`, so its tests use mocks and never open sockets or SQLite files.

`repository_sync_manager.rs` (`RepositorySyncManager`, Issue #43) syncs every meeting with paired
devices. It replaces the single-meeting `SyncManager` for the daemon's P2P task:

- **On connect:** a handshake starts for each local meeting, with one `sync::State` per (peer,
  meeting).
- **Unknown meetings:** a message about a meeting this device lacks is answered from an empty
  in-memory document. When the peer's changes arrive, `MeetingCrdtService::create_meeting_from_peer_document`
  creates the meeting, so the repository keeps the peer's origin and projects its transcript,
  segments, and title.
- **Memory bound:** at most `MAX_PENDING_REMOTE_MEETINGS` unknown meetings are held at once.
  Beyond that a message is refused, so even a misbehaving paired device cannot grow memory without
  limit.
- **Local changes:** `notify_local_change(meeting_id)` sends a local write, or a new local meeting,
  to every connected peer.
- **Disconnect:** an expired peer's state is dropped.
- **Shared adapter:** the manager borrows the network on each call rather than owning it, so the
  P2P task can share one adapter with `PairingManager`. Syncing with a device starts from the
  transport's `PeerDiscovered`, which arrives once the device is connected and trusted.
  `peer_connected` ignores a peer already recorded as connected.
- **Lost messages:** `DeliveryFailed` from a connected peer is collected (`take_failed_peers`).
  `restart_peer` drops that peer's sync states and sends a fresh handshake for every meeting.
  Without it, a dropped message leaves Automerge waiting for a reply that never comes.

`repository_sync_manager_tests.rs` runs two document-backed SQLite repositories over a linked
in-process network.

`log_control.rs` (`LogControl`, Issue #46) is the port the HTTP Settings routes use to read and
change the running daemon's log level and find its log directory. `LiveLogControl` in
`adapters/observability` implements it.

`p2p_service.rs` (`P2pService`, Issue #43) is what the daemon's P2P task owns. It joins
`PairingManager` and `RepositorySyncManager` on one transport:

- `process_pending_events` runs after every transport event. It applies sync messages and
  pairing events, and a device whose pairing completes starts syncing at once.
- `handle_command(P2pCommand)` carries out the owner's steps and local-change notices: make
  pairable (`PAIRING_WINDOW`), start, confirm, cancel, accept, reject, revoke, and `LocalChange`.
- `status()` returns a `P2pStatus` snapshot: this device, paired devices (cached, so no storage
  read), nearby devices, pairings in progress with their codes (`PairingManager::incoming_requests`
  and `outgoing_pairings`), and recent updates.

Two cases need a restart around pairing:

- **Pairing completes:** the service restarts sync with the device. This covers a device that
  already trusted this one (re-pairing after a revocation on one side), whose first handshake the
  other side dropped.
- **Delivery fails:** the service schedules one restart per device after `DEFAULT_RETRY_DELAY` (2 s).
  This covers the responder, whose messages sent right after accepting can be dropped while the
  initiator has not confirmed. `next_retry_at` tells the task loop when to wake. Failures in
  between collapse into that one restart.

`poll_local_changes`, run by the task about once a second, sends every meeting written since the
last call to the connected peers. It reads `NoteRepository::document_changes_since`, so it sees
writes from any connection or process: the recording pipeline, HTTP, the CLI, and the app. It also
sees documents merged from one peer, which it forwards to the others, so a device connected only
through another still gets changes at once.

`device_manifest_exchange.rs` frames this device's `DeviceManifest` as `NTM1` + JSON and
validates received ones (at most 16 KiB and 16 models; model labels 1–64 characters without
control characters). `P2pService` routes network events itself: a manifest message is stored
for its sender (only when the manifest names that sender), every other event goes to
`RepositorySyncManager::handle_network_event`. Each paired device that connects is sent this
device's manifest, built from `LocalCapabilities` (CPU cores, GPU memory, and the models the
pipeline is configured to run, `PipelineConfig::advertised_transcription_models`). A device's
manifest is forgotten when it disconnects or is unpaired, so the status only lists devices that
could take work now (REQUIREMENTS 3.4).

`pair_key_exchange.rs` frames this device's X25519 public key as `NTK1` + 32 bytes (Issue #50).
Given a `PairAgreementSecret` (`with_pair_agreement_secret`; the daemon loads it from the peer
store), `P2pService` sends it to each paired device that connects and derives the pair key from
the one it receives, pinning it with `TrustedPeerStore::pin_pair_key`. Malformed or low-order
keys, keys from unpaired devices, and keys that differ from the pinned one are logged and
ignored. See `crypto/README.md`.

`voice_library_exchange.rs` encodes the `NTV1` messages that copy the voice library (identities,
variants, deletion records) to a paired device, sealed with the pair key and split to fit the
transport. `P2pService` sends the library when its digest differs from what a device was last sent;
`SqliteVoiceVectorDB` merges it (#56).

`deletion.rs` is the delete use case for the CLI and HTTP API. Through `MeetingDeletionHooks` it refuses
a meeting being recorded and removes private attachment files. `remove_leftover_voices` runs when
the app and the daemon start, before P2P (#83): in each finished recording whose transcript points
at a final-pass voice, it deletes, with deletion records, the unknown voices no line points at.

`shared_prompts_exchange.rs` (#145) encodes the `NTO1` messages that copy the owner's summary and
chat prompts, sealed with the pair key; the receiver checks each prompt and keeps the newest edit
of each (`SharedPrompts::merge`). `NTS1` was taken by the meeting sync envelope; a test checks the
headers do not overlap.

`deletion_exchange.rs` encodes the `NTD1` messages listing deleted meetings and unknown voices. They
hold identifiers only. `P2pService` sends them whenever a device has not had every record, and
`RepositorySyncManager` drops sync messages about a deleted meeting.

`cluster_share_exchange.rs` encodes a `ClusterShare` (an unresolved unknown-speaker cluster, its
member embeddings, and its review clips) as sealed `NTC1` messages for one paired device
(Issue #50). The body is JSON with the clip audio appended raw, sealed with the pair key and bound
to sender and receiver. Clips are split so each message stays within an audio budget, and every
message repeats the cluster and its chunks, so each can be imported alone. Decoding bounds and
checks everything a peer controls.

`P2pService::poll_local_changes` also shares clusters: for each connected paired device with an
agreed pair key, it compares the repository's `unresolved_cluster_digests` with what that device
was last brought up to and sends only the chunks and clips it has not been sent and has not sent
here (8 MiB of audio per message), so copies never echo. Received shares are imported with
`import_cluster_share`; one whose meeting has not synced yet is held in memory (32 MiB of audio at
most, 5 minutes) and retried each tick. The record of a device is reset when it reconnects.

`poll_trust_changes`, on the same tick, calls `PairingManager::reconcile_trusted_peers`. The
stored allowlist is authoritative: a device another process removed from it (the CLI's `devices
revoke`) is dropped from the transport's gate and disconnected, a stored device not yet admitted
is admitted, and the status snapshot is refreshed when anything changed.

`MeetingCrdtService::apply_sync_message` saves only when a message added changes. Without that, a
handshake between converged devices would advance the revision and the two devices would echo each
other every second. A device that comes online later gets every meeting from whichever device it
reaches, through the connect-time handshake.

Errors about one event or peer are logged and processing continues. `p2p_service_tests.rs` pairs
two services over real memory-transport swarms and checks that a meeting, then a rename, reach the
other device.

`meeting_crdt.rs` owns the meeting document schema. The root map holds `note_text` and, since
Issue #39, an optional `title` scalar string. `create_titled_meeting` and `set_meeting_title` write
the CRDT first and then the SQLite `meetings.title` column, which is only a local projection kept
for fast list and detail reads. Every merged peer change (`apply_sync_message`,
`merge_remote_document`) projects the winning title back into SQLite.

- **Conflicts:** `title` is an Automerge register, not `Text`. Concurrent renames resolve to the
  operation with the highest Lamport counter, with ties broken by actor ID, so every peer
  converges on the same whole title. A rename made after seeing another always wins.
  Character-level `Text` merging was rejected because two concurrent renames would interleave into
  a title nobody typed.
- **Legacy documents:** a document without `title` syncs unchanged. Its meeting stays untitled and
  shows the dated `display_title()` fallback, which is never written into the CRDT, because as a
  real value it could beat a peer's real title.
- **SQLite-only titles:** a title stored only in SQLite (Issue #35) is copied into a document with
  no `title` key the first time a sync message is generated.
- **Invalid peer values:** a value that is not a valid `MeetingTitle` is ignored. It is never
  projected and never fails the sync.
- **Cost:** a rename is one `put`; a no-op rename adds no change. Tests measure about 250 bytes for
  the change-carrying message and 720 bytes for a whole fresh handshake.

`attachment_exchange.rs` encodes the sealed `NTA1` lists of attachments a device holds, and
`attachment_sync.rs` acts on them (#57): what a device lacks becomes wanted, and once its meeting has
synced the file is pulled one 64 KiB chunk at a time, at most two files at once, with a row written
only after verification. Nothing is persisted: lists are sent again on each connection, so a device
fetches what it missed when it reconnects, restarting any file that was cut off.

`transcription_exchange.rs` encodes the sealed `NTT3`/`NTT2` messages that transcribe on a paired
device, and `transcription_routing.rs` is the pipeline's engine (#59): per chunk it registers the
connected manifests with `ComputeDispatcher`, follows the owner's choice, and falls back to the local
engine on any remote failure, with the audio intact. A paired device receives the prompt
(previous text, then the glossary) as the job's `previous_text`.

`paired_chat_exchange.rs` encodes the sealed `NTQ1`/`NTQ2` messages that let a phone's chat be
answered by a paired computer (#107): the question, scope and recent turns go out; `Accepted`, then
`Answered` or `Declined` (with an error code) come back. `paired_chat_relay.rs` asks the connected
computers one at a time: a computer that does not offer it declines (`not_offered`), one silent for
20 s is skipped, and an accepted question may take up to 6 minutes. `p2p_paired_chat.rs` wires both
into the P2P service: it sends the relay's questions, and hands paired devices' questions to the
`PairedChatHost`, declining with `not_offered` when there is none.

`glossary_transcription.rs` wraps any engine so each call carries the owner's glossary and comes back
without its echo; the after-meeting pass uses it, as its engine is loaded outside the pipeline.

`ai_config.rs` keeps per-device defaults and optional task overrides provider-neutral. `ai_registry.rs`
registers only `TranscriptionEngine` trait objects and selects an explicit provider or its fallback
chain; local is preferred when configured. OpenAI remains adapter-only, while Anthropic is reserved
for future text/LLM work and currently returns `ProviderUnsupported` for transcription.

`ComputeDispatcher` retains the last successful transcript per meeting in an in-memory rolling
buffer. Its next transcription request receives that text through `TranscriptionContext`, while a
first chunk or a different meeting receives empty context. The buffer is not persisted, logged, or
shared over the network.

`voice_identity_manager.rs` depends only on `VoiceVectorDB`, `VoiceEmbedding`, and
`VoiceMatchResult`. The storage adapter calculates similarity; the manager classifies the best
returned candidate as strong only above `0.80`, weak from `0.50` through `0.80` inclusive, and no
match below `0.50`. A confirmed weak match creates a UUID-distinct embedding variant under the
user-confirmed existing identity, allowing microphone and room changes to be learned without
changing the identity.

`diarization_processor.rs` receives opaque audio bytes and a meeting ID, then coordinates only the
`DiarizationEngine`, `VoiceIdentityMatcher`, and `NoteRepository` ports. It turns each engine
segment into either a strong identity attribution or a temporary `UnassignedVoiceClip`. Weak and
absent matches retain the source bytes plus a millisecond range so manual review can play the
relevant interval and assign an identity later; the processor never logs audio or embeddings.

## Telemetry collection

`ComputeDispatcher` optionally owns a `MetricsRepository` port. It measures only the selected
`TranscriptionEngine` call; engines return their normal result and never receive telemetry types.
After a successful result, the dispatcher saves a `TaskMetrics` record with device, task/model
labels, duration, end-to-end latency, and completion timestamp. It uses a documented coarse power
heuristic: CPU-only manifests use 5 W and manifests advertising GPU VRAM use 25 W, converted to
mWh from measured milliseconds. The heuristic is an aggregate planning signal, not a battery
measurement. SQLite aggregation returns total processing time and integer-truncated average
latency without retaining audio, transcript content, or embeddings.

## Attachment meeting suggestions

`AttachmentManager` receives a local file path but delegates timestamp acquisition exclusively to
`MetadataExtractor`; it never reads EXIF, PDF, or filesystem metadata directly. It passes that UTC
timestamp to `NoteRepository::get_meetings_overlapping`, then ranks each inclusive candidate window
by its midpoint. Confidence is `1 - distance_from_midpoint / half_window_duration`: an exact midpoint
scores `1.0`, either edge scores `0.0`, and intermediate times decay linearly. Results are sorted by
descending score with meeting ID as a deterministic tie-breaker. The manager logs only processing and
suggestion counts, never paths or file contents, and returned candidates remain subject to user
confirmation.

## External meeting bridge and guaranteed identity

`TeamsProcessor` is provider-neutral application orchestration for external meeting bridges. It
subscribes only through `MeetingBridge`, passes each isolated `ExternalMediaChunk` to an
`IdentityMapper`, derives a voice vector through `VoiceEmbeddingExtractor`, stores a fresh variant
through `VoiceVectorDB`, and forwards the chunk to `TranscriptionEngine`. It does not import or
name a Microsoft, Graph, Azure, or bot SDK.

`TeamsIdentityMapper` treats `ExternalParticipant.external_id` as the durable mapping key. An
unseen key creates a new UUID `SpeakerIdentity` whose initial name is the participant's
`display_name`; an existing key returns its existing local identity unchanged. Because an external
bridge guarantees one participant per audio stream, this flow bypasses the local
diarization/weak-match/manual-confirmation path and enrolls a new embedding variant automatically.
`SlideCapture` values route only through `AttachmentWorkflow`, preserving an adapter boundary for
the attachment manager and keeping image bytes out of logs.

## Local screen-share slides (Issue #49)

`ScreenShareMonitor::sample` runs one sampling step for the meeting being recorded. When the
`ScreenShareDetector` reports a share, it captures a frame, and `SlideChangeDetector` decides
whether the frame is a new slide. New slides go to the `MeetingSlideSink` with the meeting time.
Without a share nothing is captured. A share that ends, or one from another app, starts over. A
slide that could not be stored is saved at a later stable sample.

`SlideChangeDetector` follows lecture-video slide extraction: block-based differencing on a
downscaled grayscale frame. It is based on these sources:

- [KAIST](https://koasas.kaist.ac.kr/handle/10203/203934)
- [arXiv 2411.04859](https://arxiv.org/pdf/2411.04859)
- the research comment on #49

Each frame's luma is area-averaged into a 32×18 grid. A cell has changed when it moves by at
least 8 levels, and two frames differ when at least 6 cells (about 1 %) changed. A frame is saved
once it is stable (it matches the previous sample) and differs from the last saved slide. The
heuristic therefore:

- skips transitions, animations and videos;
- ignores a moving cursor and compression noise;
- treats a new bullet line as a new slide;
- saves a slide again when the speaker goes back to it.

Only the grids are kept, never the frames. The constants are starting points to tune on real
shares.

## Meeting export workflow

`export_manager.rs` orchestrates Markdown, PDF, and JSON exports. It talks only to ports:
`NoteRepository` (meeting metadata, transcript segments, flat transcript), `AttachmentRepository`,
`VoiceVectorDB` (display names only, never vectors), and `MeetingExporter`.

1. **Validate the format.** `ExportWriter::ensure_format_supported` rejects formats with no
   configured exporter (including the reserved `Srt`) with `ExportError::UnsupportedFormat`
   before anything is read or written.
2. **Select meetings.** `export_meeting` takes one ID, `export_meetings` a list, and `export_all`
   every meeting whose creation time lies in `ExportOptions::date_range` (all meetings without a
   range). An empty selection fails with `ExportError::NoMeetingsSelected`.
3. **Extract.** `extract_meeting` returns `ExportError::MeetingNotFound` for unknown meetings. It
   prefers stored `TranscriptSegment`s, labelling speakers by resolved identity name, as
   `Speaker N` for an unnamed voice (an unnamed cluster or an identity no longer in the voice
   store, numbered in order of first appearance by `SpeakerLabels`, #118), or as `Unattributed`
   (no speaker information).
   Without segments, a non-empty flat transcript becomes one unattributed segment; with neither,
   the export has an empty transcript. Attachments are loaded only when
   `include_attachments` is set. Titles equal the meeting ID because no title is stored.
4. **Stage attachments (`ExportWriter::write`).** Markdown copies sources into
   `<export stem>_attachments/` beside the file so relative links keep working when the folder is
   moved. PDF copies into a unique OS temp directory that is always deleted afterwards. JSON copies
   nothing and lists source paths. Unreadable sources are logged by attachment ID and rendered as
   unavailable. Source files are never moved or modified.
5. **Name and write.** Generated names are `{meeting date}_{label}_{export timestamp}.{ext}`
   (or `meetings-export_{count}_{timestamp}.{ext}` for batches), sanitized to `[A-Za-z0-9._-]`,
   with `-N` appended on collision. `label` is the stored meeting title, truncated to 80
   characters (Issue #35). Untitled meetings, and titles without ASCII letters or digits, use the
   meeting ID instead, because the dated fallback title would only repeat the date prefix.
   Documents always carry the display title (stored or fallback), which the renderers use as the
   Markdown heading, PDF title, and JSON `meeting.title`. An explicit destination overrides the generated name.

`ExportManager` only borrows its `ExportWriter`, so a driver can extract under a repository lock,
drop the manager, and render on a blocking thread (the HTTP API does this with `spawn_blocking`). Errors are
`anyhow` values wrapping `ExportError`, so interfaces can `downcast_ref::<ExportError>()` to map
not-found, bad-request, and internal failures.

`pipeline/` connects transcription, diarization, voice identity, and segment persistence.
`MeetingSession` turns each VAD chunk or external bridge chunk into speaker-attributed
`TranscriptSegment` rows. See `pipeline/README.md` for the flow, alignment policy, and failure
handling. `PipelineFactory` (Issue #32) is the pipeline's composition root.

## Live recordings

`recording_manager.rs` (Issue #32) runs pipeline sessions for live recordings on one dedicated
worker thread. The port traits are not `Send`, so the worker builds its `PipelineFactory` from a
`Send` builder closure and keeps it (and any loaded model) for its whole life. Callers send FIFO
commands: `begin_recording` opens a session, `submit_chunk` queues a `VadAudioChunk` without
waiting, and `end_recording` returns a `RecordingSummary` only after every earlier chunk is
processed. A failing chunk is logged and counted while the recording continues. Dropping the
manager drains an active recording before the worker exits. `with_note_repository` runs a query
on the worker's repository. Transcription never runs on a UI thread or the async runtime.

Separately from the local recording, the worker holds at most one *external* meeting session
for the Teams bridge (Issue #33). `begin_external_meeting(meeting, SharedIdentityMapper)` opens
it: it creates the meeting, resumes it when the same meeting is already open (bot reconnects),
or ends a different one. `process_external_chunk(ExternalChunkRequest)` runs
`MeetingSession::process_external_chunk` (transcription and identity mapping, no diarization)
and waits for the segment count, which gives the bridge pump its backpressure.
`end_external_meeting` closes the session and returns its summary.

## After-meeting transcript pass (#87)

`transcript_refinement.rs` re-transcribes a finished meeting. `plan_windows` joins its speech
chunks end to end into windows of up to 30 s (`REFINEMENT_WINDOW_MS`, Whisper's input length) and
maps window time back to meeting time. `refine_meeting` transcribes each window through the
`TranscriptionEngine` port, prompting it with the previous window's text, then
`reattribute_words` gives each word the speaker of the final speaker pass at its midpoint and
groups words into segments (split at pauses over 2 s, at most 20 s). Each new segment marks its
unclear words, a guessed speaker (the nearest speaker segment was itself a guess, or lies more
than 2 s away), and the overlap the recording kept with the meeting (#90, #91); the combined pass
joins every recording's overlap on the first one's timeline. Segments outside the retained
audio are kept. Nothing is saved until every window is done, and `cancelled` is polled before each
one. Saving goes through `save_transcript_segments`, so the meeting document syncs as usual.
While it still holds the speech, `refine_meeting` also cuts a review clip for each unknown voice
that has none (#116). Such a voice came only from the final speaker pass, which has no live clip
to keep. The clip is the voice's longest microphone segment inside one retained chunk, 1 to 15 s
of it. The combined pass (#88) does the same on the first recording's timeline, counting a clip
in any of the meeting's recordings.

## One meeting recorded on several devices (#88)

The owner can record a meeting on the laptop and, at the same time, on a paired phone placed near
the other person. Each device records an ordinary meeting; both sync; the app then treats them as
one meeting with one transcript built from both microphones.

- `same_meeting.rs` recognises two recordings of one conversation from their live transcripts:
  three-word runs within 15 s of each other's wall-clock time. Their spans must overlap for at
  least half of the shorter one; then they are one meeting when shared words are at least 25% of
  the shorter transcript (10% when they cover nearly the same time) and at least 15 words. The offset between the
  recordings is the median gap of the shared words. On NOTSOFAR, devices in the same meeting
  shared 60% or more and different meetings at most 3.6%; speech-timing masks could not separate
  them.
- `combined_meetings.rs` decides. `find_combinations` folds recordings into one group, under its
  first recording, for any number of devices: a recording joins through whichever recording of
  the group it resembles. Only a device that recorded one of the two compared decides.
  `combining_device` is the group's first recording made off a phone (the first phone when all
  are phones), derived from the synced meetings so every device agrees. `plan_retained_audio`
  says what each device does with its sealed speech: after `COMBINING_GRACE` (10 min) take the
  pass alone and, with P2P, keep the speech; send it to the combining device; combine every
  recording held, however late it arrives; or delete it. Nothing waits more than
  `PlanContext::keep_for`: `KEEP_FOR_COMBINING` (three weeks) unless the owner chose 1 to 60
  days (`keep_for_days`, #93). `combining_status` / `CombiningView` say how far combining
  got, for the apps. `MeetingListing` lists a combined meeting once with every device;
  `split_recordings` is the Split action; `name_recording_devices` names devices (also while
  away) and says whether they are connected.
- `combined_refinement.rs` runs the pass over every recording. Each is transcribed in 30 s windows
  as in #87. The offset is refined from words both devices wrote. Each word keeps its own device's
  speaker, mapped to the first recording's speakers by identity, else by time overlap. Each
  speaker's words come from the device that heard that speaker best (more words). A word both
  devices wrote is kept once (within 1 s, or up to 8 s when the words around it match too), the
  copy set inside its own sentence winning; stray speaker changes inside a sentence, and changes
  a word or two off a sentence end, are mended. On NOTSOFAR's far-field devices, picking a device per speaker scored the
  same as one device; merging every word made it worse (duplicates); the union bound across
  devices was 20.3% word error against 36.9% for one device, so the gain depends on placing the
  phone near the people the laptop hears worst.
- `combined_replay_tests.rs` (ignored) replays the combining from a JSON file of two devices'
  words and speaker turns, to tune `combine_sources` on a real run.
- `speech_transfer_exchange.rs` seals the speech parts (`NTR3`, at most 12 MiB of speech each,
  about an hour) and their receipts (`NTR2`) with the pair key, bound to the sender and the
  receiver. The speech is Opus as the sender keeps it, carried unread; the receiver checks that it
  decodes. Parts of raw samples from builds before Opus (`NTR1`) are not recognised.
  `p2p_speech_transfer.rs` carries them for the P2P service and translates meetings' device IDs
  to peer IDs through the device manifests.

## Unknown-speaker clustering

`unknown_speaker_clusterer.rs` groups a meeting's weak and unmatched voices so the user labels
each person once instead of once per chunk. It persists only through `NoteRepository` and enrolls
identities only through `VoiceVectorDB`.

```text
unknown/pending-review chunk (embedding, chunk_id, duration)
  │
  ├─ validate: non-empty, finite, non-zero ─────── else InvalidEmbedding (session falls back
  │                                                 to an unclustered review clip)
  ├─ load unresolved, non-empty clusters of this meeting only
  ├─ cosine(embedding, representative) per cluster (dimension mismatch → skipped)
  ├─ best score > threshold?  yes → join; representative = (rep·n + e) / (n + 1)
  │                           no  → new cluster, representative = e
  └─ save_unknown_speaker_cluster_chunk (cluster upsert + member row, one transaction)

assign_identity_to_cluster(cluster, identity)
  1. reject unknown or already-resolved clusters
  2. identity_created = identity had no variants; store every member embedding as a variant
  3. reload segments; set speaker_identity_id where unknown_cluster_id == cluster; save
  4. resolve_unknown_speaker_cluster: mark resolved + delete its review clips (raw audio)
```

**Threshold.** `ClusteringConfig` defaults to `DEFAULT_CLUSTER_SIMILARITY_THRESHOLD = 0.40`; a
chunk joins only when its similarity is strictly greater. The value must be finite and in
`(0, 1]`. Raising it splits one person across more clusters, which means more review work but
fewer wrong merges. Lowering it merges more aggressively, risking two similar-sounding people in
one cluster, so one label would mis-attribute the other person. 0.40 was tuned on ECAPA embeddings
of LibriSpeech (#44): different speakers stay below about 0.33 (99th percentile), while the same
speaker's 4 s segments stay above about 0.49 (5th percentile).
Configure it with `MeetingSession::with_clustering_config`.

**Identity creation.** `assign_identity_to_cluster` takes a `SpeakerIdentity`. It saves the
identity through `VoiceVectorDB::save_speaker_identity` before enrolling the cluster's embeddings,
then returns the reloaded, persisted identity. `identity_created` reports whether the voice store
had no record of that UUID before. `TeamsProcessor` likewise saves an unseen participant's
identity before its first variant, without renaming identities that already exist.

**Partial failures.** The assignment steps are not one transaction across ports. Variants are
stored first. If saving segments or resolving fails, a retry re-stores variants, which only adds
duplicate acoustic variants. A cluster is never marked resolved before its segments are relinked.

## Readable transcript

`meeting_transcript.rs` gives `meetings view` (CLI) and `GET /api/meetings/{id}` (HTTP) one rule
for a meeting's transcript text. A recorded meeting stores only timed segments, and the flat
`transcript` field exists only for meetings imported as text. So `readable_transcript` returns the
non-blank segments in start-time order, one per line, then falls back to the flat text, then to
`None`. Export follows the same order. Before this, both read paths showed a recorded meeting as
having no transcript.

## Person rename

`person_rename.rs` (#186) renames a saved person, as when the owner double-clicks or long-presses
a speaker's label in a transcript. The name is trimmed and must be 1–100 characters on one line.
A name another saved person has is refused (`NameTaken`): two people with one name could not be
told apart. The app then offers to merge them (`person_merge.rs`). Saving the identity refreshes `updated_at`, so
paired devices take the newest name (#56). Names are never logged.

## Person merge

`person_merge.rs` (#188) merges one saved person into another, when the same person was saved
twice. It relinks every transcript passage that names the person merged, in every meeting, and
only then calls the voice store's `merge_identity`, which in one transaction reseals their voice
samples under new IDs for the person kept, moves external links, and deletes them with a
tombstone. In that order a failure part-way never leaves passages naming a deleted person.
Meetings and the voice library sync carry the result to paired devices.

## Meeting rename

`meeting_rename.rs` (Issue #38) is the one use case behind `meetings rename` (CLI),
`PATCH /api/meetings/{id}` (HTTP), and `note_taker_rename_meeting` (FFI).

- `rename_meeting` validates the text with `MeetingTitle::parse` before any I/O, so an invalid
  title never touches storage.
- It loads the meeting's metadata (an unknown meeting is `MeetingNotFound`), then writes the title
  through `MeetingCrdtService::set_meeting_title`: the replicated Automerge title first (Issue
  #39), then the SQLite projection. Peers receive the rename on their next sync.
- It returns `MeetingRename` with the previous display title (the dated fallback when the meeting
  was untitled) and the new title, so adapters can confirm the change. Titles are never logged.

## Cluster review

`cluster_review.rs` is the one use case behind the CLI (`voice list-clusters`,
`voice assign-cluster`), HTTP (`GET /api/meetings/{id}/unknown-clusters`,
`POST /api/clusters/{id}/assign`), and FFI (`note_taker_get_unknown_clusters`,
`note_taker_assign_cluster`) commands. Those driving adapters only parse and format.

- `load_voice_library` groups every meeting's unresolved clusters by voice (#82): average linkage
  over their representative voices, at `DEFAULT_VOICE_GROUP_SIMILARITY` (0.5). Grouping is
  recomputed on each listing, never stored or synced. `assign_clusters_to_named_identity` names a
  group's clusters one by one to one identity, skipping any already named or gone, so each cluster
  still resolves as paired devices expect.
- `list_unknown_clusters` calls `get_clusters_for_meeting`. It returns `UnknownClusterSummary`
  values (ID, meeting, total duration, chunk count, creation time) and drops the biometric
  representative vector.
- `assign_cluster_to_named_identity` trims the name and rejects a blank one before writing
  anything. A name matching an identity from `VoiceVectorDB::list_speaker_identities` (trimmed,
  ignoring case) reuses that identity. Any other name gets a fresh UUID. It then calls
  `assign_identity_to_cluster` and returns a `NamedClusterAssignment`.

Both functions accept unsized (`dyn`) ports, as the HTTP server holds them, through the
`&T` forwarding impls of `NoteRepository` and `VoiceVectorDB`. With a persistent voice store
(`SqliteVoiceVectorDB`, as composed by the CLI, daemon, and FFI), an assigned name survives
restarts and is reused by later assignments.

## Embedding encryption

`crypto/` (Issue #31) holds `VoiceEncryptor`, which seals embeddings with AES-256-GCM and
implements the `EmbeddingCipher` port, and `KeyManager`, which resolves the key from
`NOTE_TAKER_ENCRYPTION_PASSPHRASE` (Argon2id) or a random key and verifies it against
the database's key-check value. Since Issue #40 the random key lives in the platform keychain
through the `KeychainStore` port, with automatic migration from, and fallback to, the local key
file. Storage adapters receive the cipher through the port and never
handle keys. The scheme and the key-storage tradeoff are documented in `crypto/README.md`.

`meeting_summary.rs` (#95) asks a language model for a meeting's summary. The rendered template is
the instructions; the owner's notes and the speaker-attributed transcript (`[hh:mm:ss] Name: text`)
go in the user message, with photos attached. A transcript longer than the connection's context
(sized at 3 characters per token) is split into parts, the model takes notes on each, and the
summary is written from those notes, so nothing is cut. A server that cannot read images is asked
once more without them. `meeting_summary_document.rs` keeps the summary (`summary` map), the
meeting's own prompt (`summary_prompt`) and a device's request that a paired computer write the
summary (`summary_request`, #107: waiting, working or failed) in the meeting document, so all of
them sync and are sealed at rest; writes go through `NoteRepository::edit_crdt_document`, one
SQLite transaction.

`meeting_edits.rs` (#96) holds the owner's edits: the Markdown notes (`note_text`), the summary's
text (marked `edited`, or written by hand when there is none), and single transcript segments,
which keep their speaker, timing and overlap mark and lose their uncertain-word marks. A segment
edit sets the document's `transcript_edited` flag; `refinement_worker.rs` reads it and no longer
re-transcribes that meeting.

`pass_comparison.rs` (#87) logs a meeting's transcript before and after the after-meeting pass,
as `[hh:mm:ss] speaker: text` lines, at the `trace` log level only, so the owner can evaluate
the pass. At other levels nothing is read or logged.

`meeting_handoff.rs` (#114) hands a meeting to the owner's own AI app (Claude, ChatGPT), whose
login Koalino never holds. `prepare_handoff` builds the message (the summary prompt, or an
introduction for a chat) and the meeting as a Markdown file. Photos and files go beside it as
files; the Markdown only names them. `store_external_summary` keeps the
answer as the summary, marked `edited` and labelled with its app, and returns the summary it
replaced; `restore_summary` puts that one back (Undo).

`search.rs` (#100) finds the owner's words. Text is sealed at rest (#92), so it reads each
meeting (title, summary, notes, transcript) and each project file's text through the repository
and matches in memory, keeping nothing. Matching ignores case and accents (`fold`) and needs
every word of the query in one passage: a transcript segment, or one line of other text. Scopes
are every meeting, every project (names, files, meetings), or one project. Hits come with a
snippet split into matched and unmatched parts, at most `MAX_HITS` (200). Speaker names and file
text come from the host (`SearchSources`).

`notes_chat.rs` (#97) asks a language model about the owner's notes. The host gathers the
material as `ContextItem`s, most important first; a meeting has a short version (summary and
notes) beside the whole (with the transcript). `fit_context` gives everything when it fits.
Otherwise it gives short versions in order while they fit (the last items are left out), then
whole ones again while they fit; a single item larger than the context is cut. `relevance`
orders material for a question by the question's words it contains. `ask` puts the material in
the instructions, then the chat's most recent turns and the question, and returns the answer
with what was included and left out. A server that cannot read images is asked again without
them.
