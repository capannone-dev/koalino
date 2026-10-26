<!-- Documents the responsive telemetry dashboard App Router route. -->

# Telemetry dashboard route

`page.tsx` loads initial privacy-safe telemetry from the local Axum daemon. Interactive chart
refresh, WebSocket reconnecting, and responsive rendering live in the reusable
`TelemetryDashboard` component so this route remains a server-side composition boundary.
