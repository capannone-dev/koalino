// Typed client for the daemon's chat with notes (#97): one meeting, one project, or every
// meeting, summary and project. Questions and answers are never logged.

import { apiBaseUrl } from "@/lib/api";

/** What a chat is about. */
export type ChatTopic = { scope: "meeting" | "project"; id: string } | { scope: "all" };

/** A place an answer cites (#110): a meeting, at a moment when given, or a project file. */
export type ChatSource = {
  /** The number the answer cites it by, as in `[1]`. */
  number: number;
  /** Such as `Meeting "Review"` or `File plan.md`. */
  label: string;
  /** Milliseconds from the meeting's start; null for the whole meeting or a file. */
  at_ms: number | null;
  meeting_id?: string;
  project_id?: string;
  resource_id?: string;
};

/** Formats a meeting moment as `m:ss`, or `h:mm:ss` past an hour. */
function formatMoment(milliseconds: number): string {
  const totalSeconds = Math.floor(milliseconds / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}` : `${minutes}:${seconds}`;
}

/** A source as shown, such as `[1] Meeting "Review" · 1:05`. */
export function sourceLabel(source: ChatSource): string {
  const name = `[${source.number}] ${source.label}`;
  return source.at_ms === null || source.at_ms === undefined ? name : `${name} · ${formatMoment(source.at_ms)}`;
}

/**
 * Where a source opens: the meeting page at the cited moment (`#at-<ms>`, which the transcript
 * scrolls to), or the file on its project's page. Null for a file outside any project.
 */
export function sourceHref(source: ChatSource): string | null {
  if (source.meeting_id) {
    const page = `/meetings/${encodeURIComponent(source.meeting_id)}`;
    return source.at_ms === null || source.at_ms === undefined ? page : `${page}#at-${source.at_ms}`;
  }
  if (source.project_id && source.resource_id) {
    return `/projects/${encodeURIComponent(source.project_id)}#file-${source.resource_id}`;
  }
  return null;
}

/** One message, as the daemon keeps it. */
export type ChatMessage = {
  role: "user" | "assistant";
  text: string;
  created_at: string;
  /** For an answer: what the model was given. */
  included: string[];
  /** For an answer: what was left out for length. */
  omitted: string[];
  /** For an answer: the connection's label. */
  connection: string | null;
  /** For an answer: the places it cites, in order of first mention (#110). */
  sources?: ChatSource[];
};

/** A failed request: the daemon's error code (or `network`) and the HTTP status. */
export class ChatApiError extends Error {
  constructor(
    readonly code: string,
    readonly status: number | null,
    /** The AI server's own reason for a rejection (`error_detail`); shown, never logged. */
    readonly detail: string | null = null
  ) {
    super(`Chat request failed (${status ?? "network"}): ${code}`);
    this.name = "ChatApiError";
  }
}

/** Plain messages per error code; server text is not shown. */
const messages: Record<string, string> = {
  network: "Can't reach the note-taking server. Check that it is running.",
  empty: "Type a question.",
  too_long: "The question is too long.",
  no_connection: "Add an AI connection in Settings, or choose a default one.",
  connection_not_found: "That AI connection no longer exists.",
  nothing_to_chat: "There is nothing to talk about yet: no transcript, summary, notes or files.",
  context_too_small: "The AI connection's context is too small. Choose one with a larger context.",
  meeting_not_found: "That meeting no longer exists.",
  project_not_found: "That project no longer exists.",
  llm_unreachable: "The AI server could not be reached. Check the connection.",
  llm_unauthorized: "The AI server refused access. Use a server that needs no API key.",
  llm_rate_limited: "The AI server is busy. Try again later.",
  unavailable: "This server has no AI connections.",
  storage_failed: "The notes could not be read."
};

/** Words a failure for people. */
export function chatErrorMessage(error: unknown): string {
  if (error instanceof ChatApiError && error.code === "llm_rejected" && error.detail) {
    return `The AI server rejected the request. The server said: ${error.detail}`;
  }
  if (error instanceof ChatApiError && error.code === "llm_tool_failed" && error.detail) {
    return `Claude Code or Codex could not answer. It said: ${error.detail}`;
  }
  const code = error instanceof ChatApiError ? error.code : "server";
  return messages[code] ?? (code.startsWith("llm_") ? "The AI server could not answer. Try again." : "Something went wrong. Try again.");
}

/** The query string naming a chat. */
function topicQuery(topic: ChatTopic): string {
  const query = new URLSearchParams({ scope: topic.scope });
  if (topic.scope !== "all") {
    query.set("id", topic.id);
  }
  return query.toString();
}

/** Sends one request, mapping failures to `ChatApiError`. */
async function request<Answer>(path: string, init: RequestInit): Promise<Answer> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, init);
  } catch {
    throw new ChatApiError("network", null);
  }
  const body = (await response.json().catch(() => ({}))) as { error_code?: unknown; error_detail?: unknown };
  if (!response.ok) {
    const detail = typeof body.error_detail === "string" && body.error_detail !== "" ? body.error_detail : null;
    throw new ChatApiError(typeof body.error_code === "string" ? body.error_code : "server", response.status, detail);
  }
  return body as Answer;
}

/** Reads a chat's messages, oldest first (`GET /api/chat`). Throws `ChatApiError`. */
export async function getChat(topic: ChatTopic): Promise<ChatMessage[]> {
  const answer = await request<{ messages: ChatMessage[] }>(`/api/chat?${topicQuery(topic)}`, { cache: "no-store" });
  return answer.messages;
}

/**
 * Asks a question with the default AI connection (`POST /api/chat`).
 *
 * Outputs: the question and the answer as kept. Throws `ChatApiError`. Side effects: the
 * question, the recent messages and the notes that fit go to the connection's server.
 */
export async function askChat(topic: ChatTopic, question: string): Promise<{ question: ChatMessage; answer: ChatMessage }> {
  return request("/api/chat", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ ...topic, question })
  });
}

/** Deletes a chat (`DELETE /api/chat`). Throws `ChatApiError`. */
export async function clearChat(topic: ChatTopic): Promise<void> {
  await request(`/api/chat?${topicQuery(topic)}`, { method: "DELETE" });
}
