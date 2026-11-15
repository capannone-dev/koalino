// Renders the header's primary navigation. Telemetry (Issue #53) and Devices (Issue #84) are not
// here: they are pages of Settings, since owners open them rarely.

"use client";

import Link from "next/link";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";

/** The pages reachable from every page, in display order. */
const primaryPages = (t: Translate) => [
  { href: "/meetings", label: t("meetings") },
  { href: "/projects", label: t("projects") },
  { href: "/chat", label: t("chat") },
  { href: "/voice-gallery", label: t("voiceGallery") },
  { href: "/settings", label: t("settings") }
] as const;

/** The header's links; no state, no requests. */
export function PrimaryNav() {
  const t = useT();
  return (
    <nav aria-label={t("primaryNavigation")} className="flex gap-4">
      {primaryPages(t).map((page) => (
        <Link key={page.href} href={page.href}>
          {page.label}
        </Link>
      ))}
    </nav>
  );
}
