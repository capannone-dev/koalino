// Provides the shared document shell and local navigation for every dashboard page.

import type { Metadata } from "next";
import Link from "next/link";
import { PrimaryNav } from "@/components/primary-nav";
import "./styles.css";

export const metadata: Metadata = {
  title: "Koalino Server",
  description: "Local dashboard for the Koalino headless daemon"
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>
        <header className="mx-auto flex max-w-7xl items-center justify-between p-4">
          <Link href="/">Koalino</Link>
          <PrimaryNav />
        </header>
        <main className="mx-auto max-w-7xl p-4">{children}</main>
      </body>
    </html>
  );
}
