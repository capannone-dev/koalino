// Verifies the dashboard renders the aggregate Axum telemetry DTO.

import { render, screen } from "@testing-library/react";
import { TelemetryCard } from "@/components/telemetry-card";

test("renders aggregated telemetry values", () => {
  render(<TelemetryCard telemetry={{ task_count: 3, total_duration_ms: 1_200, average_latency_ms: 400 }} />);
  expect(screen.getByText(/1200 ms/)).toBeInTheDocument();
  expect(screen.getByText(/400 ms/)).toBeInTheDocument();
});
