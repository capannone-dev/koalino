// Renders the Settings page's entry to its Telemetry page (Issue #53).

import Link from "next/link";

/** A card linking to `/settings/telemetry`; no state, no requests. */
export function TelemetryEntry() {
  return (
    <Link href="/settings/telemetry" className="card block max-w-2xl space-y-1 hover:bg-surface-hover focus:outline-none focus:ring-2 focus:ring-focus/30">
      <span className="block font-semibold">Telemetry →</span>
      <span className="block text-sm text-muted">How long transcription takes on each device, and its estimated energy use.</span>
    </Link>
  );
}
