<!-- Explains the meeting_session folder: parts of the meeting session split out of meeting_session.rs. -->

# meeting_session

Parts of `MeetingSession` (`../meeting_session.rs`) kept in their own files. They are child
modules, so they can use the session's private state.

| File | Purpose |
| --- | --- |
| `participant_tracks.rs` | Credits an online-meeting participant's track by the voices it carries (#103): one voice goes to the participant, several voices (a meeting room) are matched against the gallery one by one, and a long single voice is offered for the gallery (kept only on the owner's opt-in) |
