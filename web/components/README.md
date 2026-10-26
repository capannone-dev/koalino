<!-- Documents reusable presentational and client-interactive dashboard components. -->

# Dashboard components

Components render API DTOs from `web/lib/api.ts`. Client components are limited to WebSocket status
and the voice-assignment POST form; they do not access databases or Rust implementation details.

`telemetry-dashboard.tsx` owns the responsive Recharts visualizations and bounded WebSocket refresh
loop. It receives typed local API DTOs only and never reads SQLite or media files.
