// Renders aggregated privacy-safe metrics supplied by the Axum telemetry endpoint.

import type { Telemetry } from "@/lib/api";

export function TelemetryCard({ telemetry }: { telemetry: Telemetry }) {
  return (
    <section className="card" aria-label="Telemetry">
      <h2>AI telemetry</h2>
      <div className="metrics">
        <p><strong>Total transcription time:</strong> {telemetry.total_duration_ms} ms</p>
        <p><strong>Average latency:</strong> {telemetry.average_latency_ms} ms</p>
        <p><strong>Completed tasks:</strong> {telemetry.task_count}</p>
      </div>
    </section>
  );
}
