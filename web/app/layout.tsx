// Provides the shared document shell and local navigation for every dashboard page, in the
// request's language (Issue #124): the owner's choice from Settings, else the browser's.

import type { Metadata } from "next";
import Link from "next/link";
import { LiveStatus } from "@/components/live-status";
import { PrimaryNav } from "@/components/primary-nav";
import { KeyboardShortcuts } from "@/components/keyboard-shortcuts";
import { textDirection } from "@/lib/i18n";
import { I18nProvider } from "@/lib/i18n/client";
import { getT, requestLanguage } from "@/lib/i18n/server";
import "./styles.css";

/**
 * The page title and description, in the request's language.
 *
 * Inputs: none (the current request). Outputs: Next.js metadata. Side effects: reads the request.
 */
export async function generateMetadata(): Promise<Metadata> {
  const t = await getT();
  return { title: "Koalino Server", description: t("localDashboardDescription") };
}

export default async function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  const language = await requestLanguage();
  return (
    <html lang={language} dir={textDirection(language)}>
      <body>
        <I18nProvider language={language}>
          <header className="mx-auto flex max-w-7xl items-center justify-between p-4">
            <div className="flex items-center gap-2">
              <Link href="/meetings" className="flex items-center gap-2 font-semibold">
                {/* Koalino's logo (#172), copied to public/ by branding/make-icons.sh. */}
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img src="/koalino.svg" alt="" width={28} height={28} />
                Koalino
              </Link>
              {/* Whether live updates arrive, as a dot (#205); the home page used to say it. */}
              <LiveStatus />
            </div>
            <PrimaryNav />
          </header>
          <main className="mx-auto max-w-7xl p-4 pb-24 sm:pb-4">{children}</main>
          {/* Ctrl/Cmd+K, "/" and "?" on every page (#215). */}
          <KeyboardShortcuts />
        </I18nProvider>
      </body>
    </html>
  );
}
