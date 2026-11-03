// Renders the header's primary navigation. Telemetry (Issue #53) and Devices (Issue #84) are not
// here: they are pages of Settings, since owners open them rarely.

import Link from "next/link";

/** The pages reachable from every page, in display order. */
const primaryPages = [
  { href: "/meetings", label: "Meetings" },
  { href: "/projects", label: "Projects" },
  { href: "/voice-gallery", label: "Voice gallery" },
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
