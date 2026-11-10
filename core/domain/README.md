<!-- This directory contains pure business concepts with no dependencies on infrastructure or external services. -->

# Domain layer

Domain types encode meetings, recordings, transcripts, device identities, voice identities, and domain-specific failures. It must never import ports, application services, or adapters.

Voice-signature values retain only UUID metadata and model-derived floating-point embeddings. They
never retain raw audio; persistence and similarity calculation remain outside this layer.
Review clips (`UnassignedVoiceClip`) are the exception: temporary audio kept only until the
speaker is named. `ClusterShare` groups an unresolved cluster with its member chunks and clips
for copying to a paired device (Issue #50).

Attachment values retain only a user-selected local path and declared metadata. Metadata extraction,
file access, and attachment persistence remain behind ports; meeting suggestions are immutable
domain values based on portable UTC meeting windows.

`peer_trust.rs` (Issue #43) defines the following:

- `DeviceName`: 1–64 characters, trimmed, with no control characters.
- `TrustedPeer`: a peer ID, a name, and the pairing time.
- `PairKey` (Issue #50): the 256-bit key two paired devices agreed. It is wiped on drop, prints
  as redacted, and compares in constant time (`matches`).
- `PairingCode`: the six digits both devices show during numeric-comparison pairing. It is computed
  as SHA-256 over `note-taker/pairing-code/v1\0`, the smaller peer ID, a NUL, and the larger peer
  ID. The first 8 bytes are read as a big-endian integer, taken modulo 1,000,000, and zero-padded.
  A golden test pins this derivation, so every platform must reproduce it exactly.

`whisper_catalog.rs` lists the downloadable Whisper models with pinned sizes and SHA-256 hashes (#67).
`whisper_proposal.rs` proposes a smaller or larger model from this device's own speed (#68); it is pure and
`whisper_proposal_tests.rs` specifies it with fixed telemetry.
`whisper_suggestion.rs` suggests a model for a device from its memory, GPU and core speed (#68), and
`automatic_model` resolves the "Automatic" choice to a model on the device (#106).

`transcription_language.rs` defines the language a recording is transcribed in (Issue #62): AUTO or a
code whisper.cpp knows.

`network_addresses.rs` holds the relay and static peer addresses, and the pairing invite's text form.
Invite text is untrusted, so parsing bounds every part (Issues #64 and #65).

`projects.rs` (Issue #85) defines projects, folders of meetings and resources: `ProjectId`, `ProjectName` (1–200
characters, trimmed, no control characters), `Project` with the rule paired devices use to agree on a name
(`supersedes`: newest change, then greater name), and `ProjectTombstone`, the deletion record that always wins.

`email.rs` (Issue #85) describes an imported e-mail: `EmailMessage` (subject, people, sending time, text body,
attachments) with `EmailPerson` and `EmailAttachment`, and the limits that bound a file the user picked
(50 MiB, a million body characters, 100 attachments).

`transcription_glossary.rs` holds `TranscriptionGlossary`, the owner's names and terms (people,
projects, acronyms) that steer Whisper. Terms are trimmed and kept once regardless of case; one may
not exceed 60 characters or hold a control character, and the prompt they make (`Marco Rossi,
Kubernetes, OKR.`) may not exceed 500 characters, about Whisper's 220-token prompt. The list carries
no words of any language, so it does not pull language detection towards English.
`leading_echo_words` recognises Whisper writing the prompt back (two or more terms opening a
transcript) so `without_echo` can drop it. `TranscriptionContext::prompt` (`models.rs`) puts the
previous chunk's text first and the glossary last, because Whisper keeps the end of a long prompt.

`pcm_wav.rs` (#102) wraps mono 16-bit PCM in a 44-byte RIFF/WAVE header (`wav_from_pcm16_le`).
Online-meeting bridges deliver raw PCM, but the diarization port reads WAV like the app's own
recordings; `ExternalMediaChunk::wav_bytes` (`models.rs`, 16 kHz) uses it. `pcm_wav_tests.rs`
specifies the header.

`participant_track_voices.rs` (#103) judges an online-meeting participant's audio track.
`TrackVoicePolicy::classify_chunk` groups a chunk's diarized turns by voice (cosine above 0.40, the
unknown-speaker clustering cut-off). It answers `OneVoice` when one voice holds 90% of the speech
or the others stay under 2 s, otherwise `SeveralVoices`. `ParticipantTrackVoices::observe` follows
the track: it keeps the participant while one voice speaks. It switches for good to
`SeparateVoices` on a chunk with several voices, or on a single voice unlike the track's voice once
that is known (5 s). `enrolment_candidate` offers the voice once, after 30 s of clean speech
(overlap excluded), never for several voices. Embeddings are never logged.

`transcript_confidence.rs` (#90, #91) judges how far a transcript can be trusted. Each
`TranscriptSegment` carries a `SegmentConfidence` with three marks:
- `overlapping_speech`: the segment spends at least 0.5 s, or half of a shorter segment, inside an
  `OverlapSpan`, a stretch where diarization heard two or more voices at once;
- `uncertain_speaker`: a weak voice match, or no voice near the text;
- `uncertain_words`: positions of the words whose least sure token was under 0.4. `TimedWord`
  keeps that probability in thousandths.

With Whisper small on NOTSOFAR, 42% of the words under 0.4 were wrong, against 3.1% of those at
0.95 or more. Overlap spans closer than 250 ms merge, and those under 200 ms are dropped.

`chat.rs` (#97) defines chats with notes: `ChatScope` (one meeting, one project, or everything;
stored under `meeting:<id>`, `project:<id>` or `all`), `ChatMessage` (with what an answer
included and left out, and its connection) and the limits (4 000-character questions, 200 kept
messages per chat).

`llm.rs` (#95) describes a connection to a language model and the summary prompt. `LlmConnection`
has an id, label, `LlmKind` (`OpenAiCompatible`, covering OpenAI, Mistral and local servers such as
Ollama, llama.cpp, LM Studio and vLLM; or `Anthropic`, through its native Messages API), base URL,
model and context size (2048 to 2,000,000 tokens). `check_endpoint` accepts HTTPS always, and plain
HTTP only for loopback, private (RFC 1918, IPv6 ULA) and `localhost`/`.local`/`.lan`/`.home.arpa`
hosts. It refuses credentials in the URL. `PromptTemplate` holds the owner's summary prompt (at most
8000 characters). Placeholders are `{{title}}`, `{{date}}`, `{{time}}`, `{{duration}}`, `{{device}}`,
`{{project}}`, `{{speakers}}`, `{{glossary}}` and `{{language}}`; an unknown or unclosed one is
refused when saved. The transcript itself always goes in the user message, never in the template.
