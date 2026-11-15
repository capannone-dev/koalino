// Renders the Settings page's entry to its Telemetry page (Issue #53).

"use client";

import Link from "next/link";
import { useT } from "@/lib/i18n/client";

/** A card linking to `/settings/telemetry`; no state, no requests. */
export function TelemetryEntry() {
  const t = useT();
  return (
    <Link href="/settings/telemetry" className="card block max-w-2xl space-y-1 hover:bg-surface-hover focus:outline-none focus:ring-2 focus:ring-focus/30">
      <span className="block font-semibold">{t("telemetry")}</span>
      <span className="block text-sm text-muted">{t("howLongTranscriptionTakesOn")}</span>
    </Link>
  );
}
