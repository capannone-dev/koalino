<!-- Explains the abstraction seams of the Teams bot core. -->

# Abstractions

These interfaces and records keep the core independent of the Teams SDK, so the Graph adapter
in `TeamsBot.Media` and the NSubstitute mocks in the tests are interchangeable.

- `ICallJoiner` and `ICallSession`: join a meeting, then observe its roster, unmixed audio,
  screen share, and end. They also set the recording status and hang up.
- `UnmixedAudioFrame` and `RawVideoFrame`: copies of the media buffers in managed memory.
- `RosterParticipant` and `RosterChange`: participants with their audio media source IDs.
- `IParticipantDirectory`: resolves a media source ID to a participant.
- `IBridgeSink`: where processed media goes (the WebSocket client).
