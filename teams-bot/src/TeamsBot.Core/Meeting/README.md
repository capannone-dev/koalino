<!-- Explains the meeting lifecycle code of the Teams bot. -->

# Meeting

- `BotService`: the lifecycle of one meeting.
  - **Join:** it joins with retries, announces recording, then sends the metadata.
  - **Media:** it routes audio and screen-share frames through the processors and keeps the
    roster current.
  - **End:** on leave or when Teams ends the call, it flushes and drains the bridge.
  - Each event handler catches its own failures.
- `RosterDirectory`: a thread-safe map from media source IDs to participants.
- `TeamsJoinUrl`: parses Teams join links (classic `meetup-join` and short `meet/`) and refuses
  anything that is not an https Teams meeting link.
