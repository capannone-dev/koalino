<!-- Explains the extension's test suite. -->

# Tests (`browser-extension/tests`)

Vitest specs run in jsdom (`npx vitest run`). They cover the shared logic (`src/lib`) and the
`MeetingRecorder`. Teams markup in the DOM specs is modelled on what Teams showed in the #102
probes. The extension contexts that need a real browser (page hook, background, overlay, Teams
automation) are checked by hand in the test VMs.
