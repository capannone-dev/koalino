// Renders the Settings page's entry to the Devices page (Issue #84), which left the primary
// navigation. The page keeps its `/devices` address, so existing links still work.

"use client";

import Link from "next/link";
import { useT } from "@/lib/i18n/client";

/** A card linking to `/devices`; no state, no requests. */
export function DevicesEntry() {
  const t = useT();
  return (
    <Link href="/devices" className="card block max-w-2xl space-y-1 hover:bg-surface-hover focus:outline-none focus:ring-2 focus:ring-focus/30">
      <span className="block font-semibold">{t("devices2")}</span>
      <span className="block text-sm text-muted">{t("pairYourDevicesAndSee")}</span>
    </Link>
  );
}
