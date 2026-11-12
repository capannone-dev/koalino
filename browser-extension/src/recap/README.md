<!-- Explains the recap frame script. -->

# Recap frame (`browser-extension/src/recap`)

After a meeting, Teams shows its recap in the meeting chat: a session picker and an
**AI summary / Transcript** pair of tabs. The transcript itself is in a frame served from the
organisation's SharePoint (Stream's player, `<tenant>.sharepoint.com`). That frame's own copy of
the transcript is encrypted for the player. Only its **Download → Download as .vtt** fetches
plain WebVTT (`…/media/transcripts/<id>/streamContent`).

`recapFrame.ts` runs in SharePoint frames, and acts only in one a Teams page opened
(`embeddedInTeams`: a sub-frame whose referrer is a Teams site). It never acts on a SharePoint
page opened on its own. In such a frame:

- it keeps a port to the background (`RECAP_FRAME_PORT_NAME`), opened again when Firefox stops
  an idle background;
- on `recap-download` it presses Download → Download as .vtt (`downloadRecapVtt`). The browser
  also keeps the file in its downloads;
- it passes each WebVTT file the page hook copied in this frame to the background. The background
  hands it to the Teams tab's content script, which decides which recorded meeting it belongs to
  and sends it to Koalino once.

Nothing of the transcript is logged or kept here.
