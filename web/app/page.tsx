// Renders the server home: live Axum WebSocket status. Telemetry is under Settings (Issue #53).

import { LiveStatus } from "@/components/live-status";

export default function HomePage() {
  return (
    <section className="space-y-2">
      <h1 className="text-2xl font-bold sm:text-3xl">Koalino server</h1>
      <LiveStatus />
    </section>
  );
}
