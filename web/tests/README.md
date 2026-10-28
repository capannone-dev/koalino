<!-- Documents cloud-run component tests for dashboard rendering and user actions. -->

# Dashboard tests

Vitest and React Testing Library tests live here. Run them with `npm run test`; GitHub Actions
(`web-ci.yml`) also runs them on every change under `web/`. API calls are replaced with
`vi.mock` or a stubbed global `fetch`, so no daemon is needed.

Meeting rename tests (Issue #42):

- `meeting-title.test.ts` covers trimming, empty titles, the 200-character limit counted in code
  points, control characters, and CJK/emoji titles.
- `api.test.ts` covers the `PATCH` request and the mapping of 400, 404, 5xx, other statuses,
  non-JSON error bodies, and network failures.
- `meeting-list.test.tsx` covers the actions button and right-click menu, Escape and focus return,
  the pre-filled dialog, each validation message, saving by button and Enter, unchanged titles,
  Cancel and Escape, every failure message, the in-flight state, and the empty list.
- `meeting-title-header.test.tsx` covers the pencil beside the heading, success, and failure.
- `theme.test.ts` checks that every colour token has a dark value, that the page declares
  `color-scheme: light dark`, and that no component uses a fixed palette colour that would not
  switch.

Settings tests (Issue #46):

- `log-settings-api.test.ts` covers the level `GET` and `PUT`, the failure kinds, an unreachable
  daemon, and the log download with its file name.
- `log-settings.test.tsx` covers the selector (offered and custom levels), applying a level, a
  failed change keeping the old one, the disabled selector without a server, and Export logs
  (download and no logs yet).
- `settings-navigation.test.tsx` (Issue #53) checks that the header has no telemetry entry and
  that Settings links to `/settings/telemetry`.
