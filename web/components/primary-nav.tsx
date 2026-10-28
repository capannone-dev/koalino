// Renders the header's primary navigation. Telemetry is not here: it is a page of Settings
// (Issue #53), since owners open it rarely.

import Link from "next/link";

/** The pages reachable from every page, in display order. */
const primaryPages = [
  { href: "/meetings", label: "Meetings" },
  { href: "/voice-gallery", label: "Voice gallery" },
  { href: "/devices", label: "Devices" },
  { href: "/settings", label: "Settings" }
] as const;

/** The header's links; no state, no requests. */
export function PrimaryNav() {
  return (
    <nav aria-label="Primary navigation" className="flex gap-4">
      {primaryPages.map((page) => (
        <Link key={page.href} href={page.href}>
          {page.label}
        </Link>
      ))}
    </nav>
  );
}
