// Typed client for the daemon's chat with notes (#97): one meeting, one project, or every
// meeting, summary and project. Questions and answers are never logged.

import { apiBaseUrl } from "@/lib/api";

/** What a chat is about. */
export type ChatTopic = { scope: "meeting" | "project"; id: string } | { scope: "all" };

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
  missing_key: "The AI connection needs an API key. Add it in Settings.",
  nothing_to_chat: "There is nothing to talk about yet: no transcript, summary, notes or files.",
  context_too_small: "The AI connection's context is too small. Choose one with a larger context.",
  meeting_not_found: "That meeting no longer exists.",
  project_not_found: "That project no longer exists.",
  llm_unreachable: "The AI server could not be reached. Check the connection.",
  llm_unauthorized: "The AI server refused the API key.",
  llm_rate_limited: "The AI server is busy. Try again later.",
  unavailable: "This server has no AI connections.",
  storage_failed: "The notes could not be read."
};

/** Words a failure for people. */
export function chatErrorMessage(error: unknown): string {
  if (error instanceof ChatApiError && error.code === "llm_rejected" && error.detail) {
    return `The AI server rejected the request. The server said: ${error.detail}`;
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
