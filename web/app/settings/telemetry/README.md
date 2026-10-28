<!-- Documents the Settings → Telemetry App Router route. -->

# Telemetry page

`page.tsx` is Settings → Telemetry (Issue #53; it used to be `/dashboard`, which now redirects
here). It loads initial privacy-safe telemetry from the local Axum daemon: one row per chunk the
pipeline transcribed. Interactive chart refresh, WebSocket reconnecting, and responsive rendering
live in the reusable `TelemetryDashboard` component, so this route stays a server-side
composition boundary.
