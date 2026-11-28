// Shows a meeting's time in the reader's own time zone and words (#210), such as "Today 9:32 AM".
// The server cannot know the browser's time zone, so the first render (on the server and while
// hydrating) shows the UTC date alone, and the local wording replaces it once mounted.

"use client";

import { useEffect, useState } from "react";
import { useLanguage } from "@/lib/i18n/client";
import { formatMeetingWhen } from "@/lib/meeting-metadata";

/**
 * Renders `<time>` for an instant.
 *
 * Inputs: `createdAt`, the daemon's RFC 3339 instant; `className` for styling.
 * Outputs: the element. Side effects: none.
 */
export function MeetingTime({ createdAt, className }: { createdAt: string; className?: string }) {
  const language = useLanguage();
  const [local, setLocal] = useState<string | null>(null);
  useEffect(() => setLocal(formatMeetingWhen(createdAt, language)), [createdAt, language]);
  const instant = new Date(createdAt);
  const fallback = Number.isNaN(instant.getTime())
    ? createdAt
    : new Intl.DateTimeFormat(language, { year: "numeric", month: "short", day: "numeric", timeZone: "UTC" }).format(instant);
  return (
    <time dateTime={createdAt} className={className}>
      {local ?? fallback}
    </time>
  );
}
