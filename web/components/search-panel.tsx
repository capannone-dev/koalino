// A search box with its results (#100), used on the meetings page (every meeting's transcript,
// summary and notes), the projects page (every project's files and meetings) and a project's
// page (that project only). Each result names its meeting or project and where the match is,
// marks the matched words, and links to the passage or file.

"use client";

import Link from "next/link";
import { useId, useState, type FormEvent } from "react";
import {
  hitHref,
  hitPlaceLabel,
  searchErrorMessage,
  searchNotes,
  type SearchResults,
  type SearchScope
} from "@/lib/search-api";
import { useT } from "@/lib/i18n/client";

const buttonClass =
  "inline-flex min-h-11 items-center justify-center rounded-lg border border-line-strong px-4 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus disabled:text-faint";
const fieldClass =
  "min-h-11 w-full rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30";

export type SearchPanelProps = {
  scope: SearchScope;
  /** The field's label, such as "Search meetings". */
  label: string;
  /** The project to search, for the `project` scope. */
  projectId?: string;
};

/**
 * Searches and lists the hits.
 *
 * Inputs: see `SearchPanelProps`. Outputs: the form, a status line and the results.
 * Side effects: one request per search; nothing is stored.
 */
export function SearchPanel({ scope, label, projectId }: SearchPanelProps) {
  const t = useT();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const fieldId = useId();

  /** Runs the search. */
  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setMessage(t("searching"));
    try {
      const found = await searchNotes(query, scope, projectId);
      setResults(found);
      const count = found.hits.length;
      setMessage(
        count === 0
          ? t("nothingFound")
          : `${t("resultCount", { count })}${found.truncated ? t("moreNotShownAddWords") : ""}.`
      );
    } catch (error) {
      setResults(null);
      setMessage(searchErrorMessage(error, t));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="space-y-3">
      <form role="search" className="flex flex-wrap items-end gap-2" onSubmit={(event) => void submit(event)}>
        <div className="min-w-60 flex-1">
          <label htmlFor={fieldId} className="block text-sm font-semibold">
            {label}
          </label>
          <input
            id={fieldId}
            type="search"
            className={fieldClass}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
        <button type="submit" className={buttonClass} disabled={busy}>{t("search")}</button>
      </form>
      {/* A live region without the status role, so the page's own status line stays the only one. */}
      <p aria-live="polite" className="text-sm text-muted">
        {message}
      </p>
      {results && results.hits.length > 0 ? (
        <ol className="divide-y divide-line rounded-lg border border-line">
          {results.hits.map((hit, index) => (
            <li key={index} className="space-y-1 px-4 py-3">
              <p className="flex flex-wrap items-baseline gap-x-2">
                <Link href={hitHref(hit)} className="font-semibold text-accent-text underline-offset-2 hover:underline">
                  {hit.meeting ? hit.meeting.title : (hit.project?.name ?? "")}
                </Link>
                <span className="text-sm text-muted">{hitPlaceLabel(hit, t)}</span>
              </p>
              <p className="break-words">
                {hit.snippet.map((part, position) =>
                  part.match ? (
                    <mark key={position} className="rounded bg-warning/25 px-0.5 text-inherit">
                      {part.text}
                    </mark>
                  ) : (
                    <span key={position}>{part.text}</span>
                  )
                )}
              </p>
            </li>
          ))}
        </ol>
      ) : null}
    </section>
  );
}
