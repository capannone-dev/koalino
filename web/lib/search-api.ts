// Typed client for the daemon's search route (#100): where words were said or written in
// meetings and projects, with links back to the moment or the file. Queries are never logged.

import { apiBaseUrl } from "@/lib/api";
import { englishT, type Translate } from "@/lib/i18n";

/** What to search: every meeting, every project, or one project. */
export type SearchScope = "meetings" | "projects" | "project";

/** Where a match is. */
export type SearchPlace =
  | "meeting_title"
  | "summary"
  | "notes"
  | "transcript"
  | "transcript_text"
  | "project_name"
  | "resource_name"
  | "resource_text";

/** A piece of a snippet; `match` marks a matched word. */
export type SnippetPart = { text: string; match: boolean };

/** One match, as `GET /api/search` describes it. */
export type SearchHit = {
  place: SearchPlace;
  start_time_ms?: number;
  end_time_ms?: number;
  speaker?: string;
  meeting: { meeting_id: string; title: string; created_at: string } | null;
  project: { project_id: string; name: string } | null;
  resource: { attachment_id: string; file_name: string; mime_type: string; kind: string } | null;
  snippet: SnippetPart[];
};

/** The hits of one search. */
export type SearchResults = { hits: SearchHit[]; truncated: boolean };

/** A failed search: the daemon's error code (or `network`) and the HTTP status. */
export class SearchApiError extends Error {
  constructor(
    readonly code: string,
    readonly status: number | null
  ) {
    super(`Search failed (${status ?? "network"}): ${code}`);
    this.name = "SearchApiError";
  }
}

/** Plain messages per error code; server text is not shown. */
const messages = (t: Translate) : Record<string, string> => ({
  network: t("cantReachTheNoteTaking"),
  empty: t("typeWhatToLookFor"),
  too_short: t("typeAtLeastTwoLetters"),
  too_long: t("theSearchIsTooLong"),
  project_not_found: t("thatProjectNoLongerExists"),
  storage_failed: t("theNotesCouldNotBe")
});

/**
 * Words a search failure for people.
 *
 * Inputs: any thrown value. Outputs: a sentence. Side effects: none.
 */
export function searchErrorMessage(error: unknown, t: Translate = englishT): string {
  if (error instanceof SearchApiError) return messages(t)[error.code] ?? t("somethingWentWrongOnThe");
  return t("somethingWentWrongTryAgain");
}

/**
 * Searches meetings or projects.
 *
 * Inputs: the query, the scope, and the project for the `project` scope. Outputs: the hits.
 * Errors: `SearchApiError`. Side effects: one request; the daemon reads (and unseals) the notes.
 */
export async function searchNotes(query: string, scope: SearchScope, projectId?: string): Promise<SearchResults> {
  const parameters = new URLSearchParams({ q: query, scope });
  if (projectId !== undefined) parameters.set("project", projectId);
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}/api/search?${parameters.toString()}`, { cache: "no-store" });
  } catch {
    throw new SearchApiError("network", null);
  }
  const body = (await response.json().catch(() => ({}))) as {
    error_code?: unknown;
    hits?: SearchHit[];
    truncated?: boolean;
  };
  if (!response.ok) {
    throw new SearchApiError(typeof body.error_code === "string" ? body.error_code : "server", response.status);
  }
  return { hits: body.hits ?? [], truncated: body.truncated === true };
}

/** Formats an offset from the meeting start as `m:ss`, or `h:mm:ss` past an hour. */
export function formatOffset(milliseconds: number): string {
  const totalSeconds = Math.floor(milliseconds / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}` : `${minutes}:${seconds}`;
}

/**
 * The page and anchor a hit opens: the transcript passage (`#t-<start ms>`), the summary or the
 * notes of a meeting, or a project's file (`#file-<id>`).
 *
 * Side effects: none.
 */
export function hitHref(hit: SearchHit): string {
  if (hit.meeting) {
    const page = `/meetings/${encodeURIComponent(hit.meeting.meeting_id)}`;
    switch (hit.place) {
      case "transcript":
        return `${page}#t-${hit.start_time_ms ?? 0}`;
      case "summary":
        return `${page}#summary`;
      case "notes":
        return `${page}#notes`;
      case "transcript_text":
        return `${page}#transcript`;
      default:
        return page;
    }
  }
  const page = `/projects/${encodeURIComponent(hit.project?.project_id ?? "")}`;
  return hit.resource ? `${page}#file-${hit.resource.attachment_id}` : page;
}

/** Says where in its meeting or project a hit is, such as `1:01 · Anna` or `In plan.md`. */
export function hitPlaceLabel(hit: SearchHit, t: Translate = englishT): string {
  switch (hit.place) {
    case "meeting_title":
      return t("title");
    case "summary":
      return t("summary");
    case "notes":
      return t("notes");
    case "transcript":
      return `${formatOffset(hit.start_time_ms ?? 0)} · ${hit.speaker ?? ""}`;
    case "transcript_text":
      return t("transcript");
    case "project_name":
      return t("projectName");
    case "resource_name":
      return t("file", { fileName: hit.resource?.file_name ?? "" });
    case "resource_text":
      return t("in", { place: hit.resource?.file_name ?? "" });
  }
}
