// Provides the shared document shell and local navigation for every dashboard page.

import type { Metadata } from "next";
import Link from "next/link";
import "./styles.css";

export const metadata: Metadata = {
  title: "Note Taker Server",
  description: "Local dashboard for the Note Taker headless daemon"
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>
        <header className="mx-auto flex max-w-7xl items-center justify-between p-4">
          <Link href="/">Note Taker</Link>
          <nav aria-label="Primary navigation" className="flex gap-4">
            <Link href="/dashboard">Dashboard</Link>
            <Link href="/meetings">Meetings</Link>
            <Link href="/voice-gallery">Voice gallery</Link>
            <Link href="/settings">Settings</Link>
          </nav>
        </header>
        <main className="mx-auto max-w-7xl p-4">{children}</main>
      </body>
    </html>
  );
}
