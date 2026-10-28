// Renders the web Settings page: the entry to the Telemetry page (Issue #53), the daemon's log level
// and a download of its logs (Issue #46).

import { LogSettings } from "@/components/log-settings";
import { TelemetryEntry } from "@/components/telemetry-entry";
import { getLogLevel } from "@/lib/api";

export default async function SettingsPage() {
  const level = await getLogLevel().catch(() => null);
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">Settings</h1>
      <TelemetryEntry />
      <LogSettings initialLevel={level} />
    </section>
  );
}
