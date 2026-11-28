// Renders the primary navigation (#205). Telemetry (Issue #53) and Devices (Issue #84) are not
// here: they are pages of Settings, since owners open them rarely. On wide screens the links sit in
// the header; under 640 px they become a bar fixed to the bottom of the window, icon above word, so
// they never overflow. The current section is marked for sight and screen readers.

"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { Icon, type IconName } from "@/components/icons";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";

/** The pages reachable from every page, in display order. */
const primaryPages = (t: Translate) =>
  [
    { href: "/meetings", label: t("meetings"), icon: "meetings" },
    { href: "/projects", label: t("projects"), icon: "projects" },
    { href: "/chat", label: t("chat"), icon: "chat" },
    { href: "/voice-gallery", label: t("voiceGallery"), short: t("voicesShort"), icon: "voices" },
    { href: "/settings", label: t("settings"), icon: "settings" }
  ] as const satisfies readonly { href: string; label: string; short?: string; icon: IconName }[];

/**
 * Whether `pathname` is inside the section at `href` (the section itself or one of its pages).
 *
 * Side effects: none.
 */
function isCurrent(pathname: string | null, href: string): boolean {
  return pathname !== null && (pathname === href || pathname.startsWith(`${href}/`));
}

/** The navigation links; reads the current path, sends no requests. */
export function PrimaryNav() {
  const t = useT();
  const pathname = usePathname();
  return (
    <nav
      aria-label={t("primaryNavigation")}
      className="fixed inset-x-0 bottom-0 z-20 flex justify-around border-t border-line bg-surface pb-[env(safe-area-inset-bottom)] sm:static sm:z-auto sm:justify-end sm:gap-1 sm:border-0 sm:bg-transparent sm:pb-0"
    >
      {primaryPages(t).map((page) => {
        const current = isCurrent(pathname, page.href);
        return (
          <Link
            key={page.href}
            href={page.href}
            aria-label={page.label}
            aria-current={current ? "page" : undefined}
            className={`flex min-h-14 min-w-0 flex-1 flex-col items-center justify-center gap-0.5 rounded-full px-1 text-xs focus-visible:outline-2 focus-visible:outline-focus sm:min-h-10 sm:flex-none sm:flex-row sm:gap-2 sm:px-3 sm:text-sm ${
              current ? "font-medium text-accent-text sm:bg-accent-soft" : "text-muted hover:text-ink sm:hover:bg-surface-muted"
            }`}
          >
            <Icon name={page.icon} size={22} />
            {"short" in page ? (
              <>
                {/* The bottom bar has room for one short word (#205). */}
                <span className="max-w-full truncate sm:hidden">{page.short}</span>
                <span className="hidden sm:inline">{page.label}</span>
              </>
            ) : (
              <span className="max-w-full truncate">{page.label}</span>
            )}
          </Link>
        );
      })}
    </nav>
  );
}
