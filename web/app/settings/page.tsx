// Renders the web Settings page (Issue #46): the daemon's log level and a download of its logs.

import { LogSettings } from "@/components/log-settings";
import { getLogLevel } from "@/lib/api";

export default async function SettingsPage() {
  const level = await getLogLevel().catch(() => null);
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">Settings</h1>
      <LogSettings initialLevel={level} />
    </section>
  );
}
