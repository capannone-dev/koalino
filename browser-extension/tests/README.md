<!-- Explains the extension's test suite. -->

# Tests (`browser-extension/tests`)

Vitest specs run in jsdom (`npx vitest run`). They cover the shared logic (`src/lib`) and the
`MeetingRecorder`. Teams markup in the DOM specs is modelled on what Teams showed in the #102
probes. The extension contexts that need a real browser (page hook, background, overlay, Teams
automation) are checked by hand in the test VMs. The More menu in `automation.test.ts`, the slide frames in
`slides.test.ts` and the profile labels in `teamsDom.test.ts` copy what the Firefox live test on
the work account showed (November 2026).
