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
`whisper_suggestion.rs` suggests a model for a device from its memory, GPU and core speed (#68).

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

`transcript_confidence.rs` (#90, #91) judges how far a transcript can be trusted. Each
`TranscriptSegment` carries a `SegmentConfidence` with three marks:
- `overlapping_speech`: the segment spends at least 0.5 s, or half of a shorter segment, inside an
  `OverlapSpan`, a stretch where diarization heard two or more voices at once;
- `uncertain_speaker`: a weak voice match, or no voice near the text;
- `uncertain_words`: positions of the words whose least sure token was under 0.4. `TimedWord`
  keeps that probability in thousandths.

With Whisper small on NOTSOFAR, 42% of the words under 0.4 were wrong, against 3.1% of those at
0.95 or more. Overlap spans closer than 250 ms merge, and those under 200 ms are dropped.
