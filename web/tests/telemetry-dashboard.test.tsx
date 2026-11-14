// Verifies the telemetry dashboard presents summary metrics and known device status cards.

import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { TelemetryDashboard } from "@/components/telemetry-dashboard";

test("renders summary cards and a known device", () => {
  render(<TelemetryDashboard initialData={{ devices: [{ device_id: "device-rome", supports_vad: true, supported_models: ["whisper"], last_seen: Date.now() }], summary: { total_transcription_time_ms: 3_600_000, average_latency_ms: 250, total_tasks: 4, power_estimate_mwh: 500 }, series: [{ device_id: "device-rome", metrics: [{ task_type: "transcription", duration_ms: 1200, latency_ms: 250, power_mwh: 500, recorded_at: Date.now() }] }] }} />);
  expect(screen.getByText("01:00")).toBeInTheDocument();
  expect(screen.getByText("device-rome")).toBeInTheDocument();
});
