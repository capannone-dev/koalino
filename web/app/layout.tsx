// Provides the shared document shell and local navigation for every dashboard page, in the
// request's language (Issue #124): the owner's choice from Settings, else the browser's.

import type { Metadata } from "next";
import Link from "next/link";
import { PrimaryNav } from "@/components/primary-nav";
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
            <Link href="/">Koalino</Link>
            <PrimaryNav />
          </header>
          <main className="mx-auto max-w-7xl p-4">{children}</main>
        </I18nProvider>
      </body>
    </html>
  );
}
