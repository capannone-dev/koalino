<!-- Explains the content script and its helpers. -->

# Content script (`browser-extension/src/content`)

| File | Purpose |
| --- | --- |
| `content.ts` | Entry point: a once-a-second check of the page; consent; starting and stopping a recording; captions, transcription and slide capture while recording; recap transcripts outside calls. |
| `recorder.ts` | `MeetingRecorder`: one recording, with injected I/O. It sends the meeting, audio chunks, slides (from the hook's signatures and JPEGs), the captions and live transcript documents, and the end. The owner's microphone is the participant `browser-owner:<name>`, so each owner gets a person of their own. |
| `automation.ts` | Clicks through Teams: turn on captions, start transcription (opt-in), open a recap's Transcript tab and have the recap frame download it (`downloadRecapVtt` runs in that frame). Each step finds controls by their stable IDs, `data-tid`, ARIA label or English text (ignoring a shortcut after the label), runs one at a time, closes the More menu after itself, and gives up quietly. |
| `banner.ts` | The consent question and the recording indicator, in a closed shadow root. |
