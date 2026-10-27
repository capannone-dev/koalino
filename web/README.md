<!-- Explains the Next.js dashboard and its integration with the local Axum daemon. -->

# Note Taker Linux Server dashboard

This Next.js App Router application is the local web dashboard for the Rust headless daemon. It
consumes only documented Axum JSON/WebSocket boundaries; it never reads SQLite or local media files
directly.

## Setup and development

Install Node.js 22 or later, then run:

```bash
cd web
npm install
NEXT_PUBLIC_API_BASE_URL=http://127.0.0.1:8080 npm run dev
```

`NEXT_PUBLIC_API_BASE_URL` defaults to `http://127.0.0.1:8080`. The browser WebSocket URL can be
overridden with `NEXT_PUBLIC_WS_URL`; otherwise it derives from the API URL. Run `npm run lint`,
`npm run test`, and `npm run build` before deployment. GitHub Actions performs those checks in the
cloud because the local coding sandbox has no Node.js runtime.

## Pages

- `/dashboard` is the responsive telemetry dashboard. It renders device health, summary cards,
  duration, latency, task mix, and power charts from the local Axum API. Its WebSocket client
  refreshes a telemetry update at most once every ten seconds and reconnects after disconnect.
- `/` shows aggregate telemetry and live Axum WebSocket status messages.
- `/meetings` lists local meetings; `/meetings/<id>` shows transcript and attachment metadata.
- `/voice-gallery` loads every meeting's unresolved unknown-speaker clusters from
  `GET /api/meetings/{id}/unknown-clusters`. It shows each cluster's total speech time and chunk
  count, and posts the typed name to `POST /api/clusters/{id}/assign`. An assigned cluster is
  removed from the list.

Cluster responses deliberately carry no audio paths, so the web gallery has no playback. Local
review clips can be played from the Flutter Voice Gallery, which receives local file paths over FFI.
