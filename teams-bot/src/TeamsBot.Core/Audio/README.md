<!-- Explains the audio processing stage of the Teams bot. -->

# Audio

- `PcmConverter`: converts 16-bit PCM at any supported rate and channel count to 16 kHz mono
  (downmix plus box-filter decimation). Audio that is already 16 kHz mono passes through.
- `AudioProcessor`: keeps one open chunk per speaker.
  - It pads small gaps with silence.
  - A chunk is cut at a silence gap longer than `SilenceGapMs` (600 ms) or at
    `MaximumChunkDurationMs` (10 s; the protocol allows up to 30 s).
  - Each chunk is tagged with the participant resolved through the roster.

Raw audio stays in memory only until it has been sent.
