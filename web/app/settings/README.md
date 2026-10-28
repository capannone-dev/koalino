<!-- This directory holds the web Settings page. -->

# Settings page

`page.tsx` links to the Telemetry page (`telemetry/`, Issue #53), reads the daemon's log level (`GET /api/settings/log-level`) on the server and renders
`LogSettings` (`components/log-settings.tsx`), which changes the level
(`PUT /api/settings/log-level`, until the daemon restarts) and downloads the newest week of logs
(`GET /api/logs/export`) (REQUIREMENTS 6.1, 6.2; Issue #46). When the daemon cannot be reached the
selector is disabled and the page says so.
