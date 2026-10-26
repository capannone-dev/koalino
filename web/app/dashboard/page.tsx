// Composes initial telemetry dashboard data from documented local Axum endpoints.

import { TelemetryDashboard } from "@/components/telemetry-dashboard";
import { getDeviceTelemetry, getDevices, getTelemetrySummary } from "@/lib/api";

export default async function DashboardPage() {
  const devicesResponse = await getDevices();
  const series = await Promise.all(devicesResponse.devices.map(async (device) => getDeviceTelemetry(device.device_id).catch(() => ({ device_id: device.device_id, metrics: [] }))));
  return <TelemetryDashboard initialData={{ devices: devicesResponse.devices, summary: await getTelemetrySummary(), series }} />;
}
