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
        <header className="site-header">
          <Link href="/">Note Taker</Link>
          <nav aria-label="Primary navigation">
            <Link href="/meetings">Meetings</Link>
            <Link href="/voice-gallery">Voice gallery</Link>
          </nav>
        </header>
        <main>{children}</main>
      </body>
    </html>
  );
}
