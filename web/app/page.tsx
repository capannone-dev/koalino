// Renders the dashboard home from the Axum telemetry endpoint and live event client.

import { LiveStatus } from "@/components/live-status";
import { TelemetryCard } from "@/components/telemetry-card";
import { getTelemetry } from "@/lib/api";

export default async function DashboardPage() {
  const telemetry = await getTelemetry();
  return (
    <section>
      <h1>Server dashboard</h1>
      <TelemetryCard telemetry={telemetry} />
      <LiveStatus />
    </section>
  );
}
