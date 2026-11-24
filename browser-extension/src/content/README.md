<!-- Explains the content script and its helpers. -->

# Content script (`browser-extension/src/content`)

| File | Purpose |
| --- | --- |
| `content.ts` | Entry point: a once-a-second check of the page; consent; starting and stopping a recording; captions, transcription and slide capture while recording; recap transcripts outside calls. The tab's `sessionStorage` keeps the meeting ID it records (`koalino-recording-meeting`), so after a reload it continues that meeting (#102); on `pagehide` it interrupts rather than ends the recording. |
| `recorder.ts` | `MeetingRecorder`: one recording, with injected I/O. It sends the meeting, audio chunks, slides (from the hook's signatures and JPEGs), the captions and live transcript documents, and the end. `interrupt()` sends all but the end; a recorder given the first part's `startMs` continues its meeting times. The owner's microphone is the participant `browser-owner:<name>`, so each owner gets a person of their own. |
| `automation.ts` | Clicks through Teams: turn on captions, start transcription (opt-in), open a recap's Transcript tab and have the recap frame download it (`downloadRecapVtt` runs in that frame). A meeting chat keeps a recap per session and Teams may open any of them, so before downloading it chooses the recorded session in "Select the meeting by time" and downloads nothing when none fits (#158). Each step finds controls by their stable IDs, `data-tid`, ARIA label or English text (ignoring a shortcut after the label), runs one at a time, closes the More menu after itself, and gives up quietly. |
| `banner.ts` | The consent question and the recording indicator, in a closed shadow root. |
