<!-- Explains the content script and its helpers. -->

# Content script (`browser-extension/src/content`)

| File | Purpose |
| --- | --- |
| `content.ts` | Entry point: a once-a-second check of the page; consent; starting and stopping a recording; captions, transcription and slide capture while recording; recap transcripts outside calls. |
| `recorder.ts` | `MeetingRecorder`: one recording, with injected I/O. It sends the meeting, audio chunks, slides, the captions and live transcript documents, and the end. |
| `automation.ts` | Clicks through Teams: turn on captions, start transcription (opt-in), open a recap's transcript. Each step finds controls by `data-tid`, ARIA label or English text, and gives up quietly. |
| `banner.ts` | The consent question and the recording indicator, in a closed shadow root. |
