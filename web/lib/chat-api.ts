// Typed client for the daemon's chat with notes (#97): one meeting, one project, or every
// meeting, summary and project. Questions and answers are never logged.

import { apiBaseUrl } from "@/lib/api";
import { englishT, type Translate } from "@/lib/i18n";

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
const messages = (t: Translate) : Record<string, string> => ({
  network: t("cantReachTheNoteTaking"),
  empty: t("typeAQuestion"),
  too_long: t("theQuestionIsTooLong"),
  no_connection: t("addAnAiConnectionIn2"),
  connection_not_found: t("thatAiConnectionNoLonger"),
  nothing_to_chat: t("thereIsNothingToTalk"),
  context_too_small: t("theAiConnectionsContextIs"),
  meeting_not_found: t("thatMeetingNoLongerExists"),
  project_not_found: t("thatProjectNoLongerExists"),
  llm_unreachable: t("theAiServerCouldNot"),
  llm_unauthorized: t("theAiServerRefusedAccess"),
  llm_rate_limited: t("theAiServerIsBusy"),
  unavailable: t("thisServerHasNoAi"),
  storage_failed: t("theNotesCouldNotBe")
});

/** Words a failure for people. */
export function chatErrorMessage(error: unknown, t: Translate = englishT): string {
  if (error instanceof ChatApiError && error.code === "llm_rejected" && error.detail) {
    return t("theAiServerRejectedThe", { detail: error.detail });
  }
  if (error instanceof ChatApiError && error.code === "llm_tool_failed" && error.detail) {
    return t("claudeCodeOrCodexCould", { detail: error.detail });
  }
  const code = error instanceof ChatApiError ? error.code : "server";
  return messages(t)[code] ?? (code.startsWith("llm_") ? t("theAiServerCouldNot2") : t("somethingWentWrongTryAgain"));
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
