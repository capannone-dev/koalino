// Renders Settings → Telemetry (Issue #53): initial telemetry dashboard data from the documented
// local Axum endpoints. It moved here from `/dashboard`, which now redirects.

import Link from "next/link";
import { TelemetryDashboard } from "@/components/telemetry-dashboard";
import { getDeviceTelemetry, getDevices, getTelemetrySummary } from "@/lib/api";

export default async function TelemetryPage() {
  const devicesResponse = await getDevices();
  const series = await Promise.all(devicesResponse.devices.map(async (device) => getDeviceTelemetry(device.device_id).catch(() => ({ device_id: device.device_id, metrics: [] }))));
  return (
    <div className="space-y-4">
      <Link href="/settings" className="text-sm text-muted hover:text-ink">← Settings</Link>
      <TelemetryDashboard initialData={{ devices: devicesResponse.devices, summary: await getTelemetrySummary(), series }} />
    </div>
  );
}
