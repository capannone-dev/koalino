<!-- Documents reusable presentational and client-interactive dashboard components. -->

# Dashboard components

Components render API DTOs from `web/lib/api.ts`. Client components are limited to WebSocket status
and the cluster-assignment form; they do not access databases or Rust implementation details.

`voice-gallery.tsx` renders one card per unknown-speaker cluster with a name field and an
**Assign** button. A blank name is rejected without a request. A successful assignment removes the
card and reports the resolved name. A failed request keeps the card and shows an error message.

`telemetry-dashboard.tsx` owns the responsive Recharts visualizations and bounded WebSocket refresh
loop. It receives typed local API DTOs only and never reads SQLite or media files.
