<!-- Explains the bridge data model of the Teams bot. -->

# Model

These immutable records are exchanged between the processors and the bridge:

- `ParticipantInfo`: Teams user ID, display name, and optional e-mail.
- `MeetingInfo`: the meeting ID, title, start time, and roster.
- `PcmAudioChunk`: 16 kHz mono PCM and its start offset.
- `EncodedVideoFrame`: a JPEG and its offset.
