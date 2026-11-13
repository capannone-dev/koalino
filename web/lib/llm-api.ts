// Typed client for the daemon's AI connection and meeting summary routes (#95). Koalino keeps no
// API keys (#117): connections are servers on the network, or Claude Code and Codex.

import { apiBaseUrl } from "@/lib/api";

/** The protocol a connection speaks; `claude_code` and `codex` run the signed-in tool on the server's computer (#105). */
export type LlmKind = "open_ai_compatible" | "claude_code" | "codex";

/** Whether a kind runs a tool instead of calling a server: no address and no key. */
export function isLocalCli(kind: LlmKind): boolean {
  return kind === "claude_code" || kind === "codex";
}

/** A saved connection. */
export type LlmConnection = {
  id: string;
  label: string;
  kind: LlmKind;
  base_url: string;
  model: string;
  context_tokens: number;
  plain_http: boolean;
  is_default: boolean;
  /** For Claude Code and Codex: whether the tool was found on the server's computer. */
  tool_installed?: boolean;
};

/** The AI settings. */
export type LlmSettings = {
  connections: LlmConnection[];
  default_connection: string | null;
  template: string;
  default_template: string;
  custom_template: boolean;
  auto_summarize: boolean;
  placeholders: string[];
};

/** What saving a connection sends. */
export type LlmConnectionInput = {
  label: string;
  kind: LlmKind;
  base_url: string;
  model: string;
  context_tokens: number;
};

/** A meeting's stored summary. */
export type StoredSummary = {
  text: string;
  connection_label: string;
  model: string;
  created_at: string;
  prompt: string;
  edited: boolean;
  truncated: boolean;
};

/** A meeting's summary and its own prompt, each null when absent. */
export type MeetingSummaryState = { summary: StoredSummary | null; meeting_prompt: string | null };

/** What writing a summary returns. */
export type SummaryResult = {
  summary: StoredSummary;
  parts: number;
  images_sent: number;
  images_skipped: number;
};

/** A failed AI request: the daemon's error code (or `network`) and the HTTP status. */
export class LlmApiError extends Error {
  constructor(
    readonly code: string,
    readonly status: number | null,
    /** The AI server's own reason for a rejection (`error_detail`); shown, never logged. */
    readonly detail: string | null = null
  ) {
    super(`AI request failed (${status ?? "network"}): ${code}`);
    this.name = "LlmApiError";
  }
}

/** Plain messages per error code; server text is not shown, since it can hold internal details. */
const messages: Record<string, string> = {
  network: "Can't reach the note-taking server. Check that it is running.",
  unavailable: "This server has no AI connections.",
  invalid_connection: "Check the address and model. Plain http:// works only on this computer or the local network.",
  invalid_template: "The prompt has an unknown or unclosed {{placeholder}}.",
  invalid_request: "The request was not understood.",
  connection_not_found: "That connection no longer exists.",
  meeting_not_found: "That meeting no longer exists.",
  no_connection: "Add an AI connection in Settings first.",
  nothing_to_summarize: "This meeting has no transcript or notes yet.",
  too_long: "This meeting is too long for this connection. Choose one with a larger context.",
  llm_unreachable: "The AI server could not be reached.",
  llm_unauthorized: "The AI server refused access. Use a server that needs no API key.",
  llm_rate_limited: "The AI server is busy or over its limit. Try again later.",
  llm_rejected: "The AI server rejected the request. Check the model name.",
  llm_failed: "The AI server failed. Try again.",
  llm_refused: "The model declined to answer.",
  llm_invalid_response: "The AI server's answer could not be read.",
  llm_tool_missing: "Claude Code or Codex is not installed on the server's computer, or it cannot be found.",
  llm_tool_failed: "Claude Code or Codex could not answer. Check that it is signed in.",
  save_failed: "The settings could not be saved.",
  storage_failed: "The meeting could not be read or saved."
};

/**
 * Words an AI failure for people.
 *
 * Inputs: any thrown value. Outputs: a sentence. Side effects: none.
 */
export function llmErrorMessage(error: unknown): string {
  if (error instanceof LlmApiError && error.code === "llm_rejected" && error.detail) {
    return `The AI server rejected the request. The server said: ${error.detail}`;
  }
  if (error instanceof LlmApiError && error.code === "llm_tool_failed" && error.detail) {
    return `Claude Code or Codex could not answer. It said: ${error.detail}`;
  }
  if (error instanceof LlmApiError) return messages[error.code] ?? "Something went wrong on the server. Try again.";
  return "Something went wrong. Try again.";
}

/** Sends one request and returns its JSON, mapping failures to `LlmApiError`. */
async function request<Answer>(path: string, init: RequestInit = {}): Promise<Answer> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, { cache: "no-store", ...init });
  } catch {
    throw new LlmApiError("network", null);
  }
  const body = (await response.json().catch(() => ({}))) as { error_code?: unknown; error_detail?: unknown };
  if (!response.ok) {
    const detail = typeof body.error_detail === "string" && body.error_detail !== "" ? body.error_detail : null;
    throw new LlmApiError(typeof body.error_code === "string" ? body.error_code : "server", response.status, detail);
  }
  return body as Answer;
}

/** Builds a JSON request. */
function json(method: string, body: unknown): RequestInit {
  return { method, headers: { "content-type": "application/json" }, body: JSON.stringify(body) };
}

/** Reads the AI settings (`GET /api/settings/llm`). */
export function getLlmSettings(): Promise<LlmSettings> {
  return request<LlmSettings>("/api/settings/llm");
}

/**
 * Changes only the fields given (`PUT /api/settings/llm`); `template: null` restores the
 * built-in one. Returns the settings as saved.
 */
export function updateLlmSettings(change: {
  default_connection?: string | null;
  template?: string | null;
  auto_summarize?: boolean;
}): Promise<LlmSettings> {
  return request<LlmSettings>("/api/settings/llm", json("PUT", change));
}

/** Creates a connection, or edits the one with `id`. Returns it, key left out. */
export async function saveLlmConnection(input: LlmConnectionInput, id?: string): Promise<LlmConnection> {
  const answer = await request<{ connection: LlmConnection }>(
    id ? `/api/settings/llm/connections/${encodeURIComponent(id)}` : "/api/settings/llm/connections",
    json(id ? "PUT" : "POST", input)
  );
  return answer.connection;
}

/** Removes a connection and its key. */
export async function deleteLlmConnection(id: string): Promise<void> {
  await request(`/api/settings/llm/connections/${encodeURIComponent(id)}`, { method: "DELETE" });
}

/** Sends a one-line test request, holding no meeting content; returns the model's reply. */
export async function testLlmConnection(id: string): Promise<string> {
  const answer = await request<{ reply: string }>(`/api/settings/llm/connections/${encodeURIComponent(id)}/test`, {
    method: "POST"
  });
  return answer.reply;
}

/** Reads a meeting's summary and its own prompt. */
export function getMeetingSummary(meetingId: string): Promise<MeetingSummaryState> {
  return request<MeetingSummaryState>(`/api/meetings/${encodeURIComponent(meetingId)}/summary`);
}

/**
 * Writes a meeting's summary with a connection, the default one when none is given. Sends the
 * meeting's transcript, notes and photos to that connection; can take minutes with a local model.
 */
export function summarizeMeeting(meetingId: string, connectionId?: string): Promise<SummaryResult> {
  return request<SummaryResult>(
    `/api/meetings/${encodeURIComponent(meetingId)}/summary`,
    json("POST", connectionId ? { connection_id: connectionId } : {})
  );
}

/** Sets the meeting's own prompt, or goes back to the default with `null`. */
export async function setMeetingPrompt(meetingId: string, prompt: string | null): Promise<void> {
  await request(`/api/meetings/${encodeURIComponent(meetingId)}/summary-prompt`, json("PUT", { prompt }));
}

/** Ready-made connections the Settings form can start from. */
export const connectionPresets: { name: string; input: Omit<LlmConnectionInput, "label"> }[] = [
  { name: "Local server (Ollama)", input: { kind: "open_ai_compatible", base_url: "http://192.168.1.10:11434/v1", model: "llama3.1:8b", context_tokens: 8192 } },
  { name: "LM Studio", input: { kind: "open_ai_compatible", base_url: "http://192.168.1.10:1234/v1", model: "qwen3-8b", context_tokens: 32768 } },
  { name: "Claude Code (Claude plan)", input: { kind: "claude_code", base_url: "", model: "", context_tokens: 200000 } },
  { name: "Codex (ChatGPT plan)", input: { kind: "codex", base_url: "", model: "", context_tokens: 200000 } }
];
