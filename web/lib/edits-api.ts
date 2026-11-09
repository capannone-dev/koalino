// Typed client for the daemon's meeting edit routes (#96): the owner's notes, the summary's text
// and single transcript segments.

import { apiBaseUrl } from "@/lib/api";
import type { StoredSummary } from "@/lib/llm-api";

/** A failed edit: the daemon's error code (or `network`) and the HTTP status. */
export class EditApiError extends Error {
  constructor(
    readonly code: string,
    readonly status: number | null
  ) {
    super(`Edit failed (${status ?? "network"}): ${code}`);
    this.name = "EditApiError";
  }
}

/** Plain messages per error code; server text is not shown. */
const messages: Record<string, string> = {
  network: "Can't reach the note-taking server. Check that it is running.",
  empty: "Write something first.",
  too_long: "The text is too long.",
  meeting_not_found: "That meeting no longer exists.",
  segment_not_found: "That part of the transcript changed. Reload the page.",
  storage_failed: "The change could not be saved."
};

/**
 * Words an edit failure for people.
 *
 * Inputs: any thrown value. Outputs: a sentence. Side effects: none.
 */
export function editErrorMessage(error: unknown): string {
  if (error instanceof EditApiError) return messages[error.code] ?? "Something went wrong on the server. Try again.";
  return "Something went wrong. Try again.";
}

/** Sends one request and returns its JSON, mapping failures to `EditApiError`. */
async function request<Answer>(path: string, init: RequestInit = {}): Promise<Answer> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, { cache: "no-store", ...init });
  } catch {
    throw new EditApiError("network", null);
  }
  const body = (await response.json().catch(() => ({}))) as { error_code?: unknown };
  if (!response.ok) {
    throw new EditApiError(typeof body.error_code === "string" ? body.error_code : "server", response.status);
  }
  return body as Answer;
}

/** Builds a JSON `PUT`. */
function put(body: unknown): RequestInit {
  return { method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify(body) };
}

/** The meeting's path segment. */
function meetingPath(meetingId: string): string {
  return `/api/meetings/${encodeURIComponent(meetingId)}`;
}

/** Reads the owner's notes, `null` when there are none. */
export async function getMeetingNotes(meetingId: string): Promise<string | null> {
  return (await request<{ notes: string | null }>(`${meetingPath(meetingId)}/notes`)).notes;
}

/** Replaces the owner's notes (Markdown); they sync to paired devices. */
export async function saveMeetingNotes(meetingId: string, text: string): Promise<void> {
  await request(`${meetingPath(meetingId)}/notes`, put({ text }));
}

/** Replaces the summary's text, or writes one by hand; returns it marked as edited. */
export async function editSummaryText(meetingId: string, text: string): Promise<StoredSummary> {
  return (await request<{ summary: StoredSummary }>(`${meetingPath(meetingId)}/summary`, put({ text }))).summary;
}

/** A corrected segment as the daemon stored it. */
export type EditedSegment = { start_time_ms: number; end_time_ms: number; text: string };

/**
 * Corrects one transcript segment, keeping its speaker and timing; blank text removes it and
 * returns `null`. The meeting is then no longer re-transcribed.
 */
export async function editSegment(
  meetingId: string,
  startTimeMs: number,
  endTimeMs: number,
  text: string
): Promise<EditedSegment | null> {
  return (
    await request<{ segment: EditedSegment | null }>(
      `${meetingPath(meetingId)}/segments`,
      put({ start_time_ms: startTimeMs, end_time_ms: endTimeMs, text })
    )
  ).segment;
}
