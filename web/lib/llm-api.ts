// Typed client for the daemon's AI connection and meeting summary routes (#95). Koalino keeps no
// API keys (#117): connections are servers on the network, or Claude Code and Codex.

import { apiBaseUrl } from "@/lib/api";
import { englishT, type Translate } from "@/lib/i18n";

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
  /** Whether an API key is saved for it (#138); the key itself is never sent back. */
  has_key?: boolean;
};

/** The AI settings. */
export type LlmSettings = {
  connections: LlmConnection[];
  default_connection: string | null;
  template: string;
  default_template: string;
  custom_template: boolean;
  auto_summarize: boolean;
  /** The connection that writes the summaries paired devices ask for (#107), or null. */
  paired_summary_connection: string | null;
  placeholders: string[];
};

/** What saving a connection sends. */
export type LlmConnectionInput = {
  label: string;
  kind: LlmKind;
  base_url: string;
  model: string;
  context_tokens: number;
  /** A new API key for an online service (#138); absent keeps the saved one. */
  api_key?: string;
  /** Deletes the saved key. */
  clear_key?: boolean;
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
const messages = (t: Translate) : Record<string, string> => ({
  network: t("cantReachTheNoteTaking"),
  unavailable: t("thisServerHasNoAi"),
  invalid_connection: t("checkTheAddressAndModel"),
  invalid_template: t("thePromptHasAnUnknown"),
  invalid_request: t("theRequestWasNotUnderstood"),
  connection_not_found: t("thatConnectionNoLongerExists"),
  meeting_not_found: t("thatMeetingNoLongerExists"),
  no_connection: t("addAnAiConnectionIn3"),
  nothing_to_summarize: t("thisMeetingHasNoTranscript"),
  too_long: t("thisMeetingIsTooLong"),
  llm_unreachable: t("theAiServerCouldNot3"),
  llm_unauthorized: t("theAiServerRefusedAccess"),
  llm_rate_limited: t("theAiServerIsBusy2"),
  llm_rejected: t("theAiServerRejectedThe2"),
  llm_failed: t("theAiServerFailedTry"),
  llm_refused: t("theModelDeclinedToAnswer"),
  llm_invalid_response: t("theAiServersAnswerCould"),
  llm_tool_missing: t("claudeCodeOrCodexIs"),
  llm_tool_failed: t("claudeCodeOrCodexCould2"),
  save_failed: t("theSettingsCouldNotBe"),
  storage_failed: t("theMeetingCouldNotBe"),
  storage_error: t("theMeetingCouldNotBe"),
  no_summary: t("noSummaryYet"),
  export_failed: t("theSummaryCouldNotBeExported")
});

/**
 * Words an AI failure for people.
 *
 * Inputs: any thrown value. Outputs: a sentence. Side effects: none.
 */
export function llmErrorMessage(error: unknown, t: Translate = englishT): string {
  if (error instanceof LlmApiError && error.code === "llm_rejected" && error.detail) {
    return t("theAiServerRejectedThe", { detail: error.detail });
  }
  if (error instanceof LlmApiError && error.code === "llm_tool_failed" && error.detail) {
    return t("claudeCodeOrCodexCould", { detail: error.detail });
  }
  if (error instanceof LlmApiError) return messages(t)[error.code] ?? t("somethingWentWrongOnThe");
  return t("somethingWentWrongTryAgain");
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

/** The formats a summary downloads as (#130), named as the daemon expects them. */
export type SummaryExportFormat = "pdf" | "txt" | "docx" | "md";

/** A downloaded summary file. */
export type SummaryDownload = { fileName: string; blob: Blob };

/**
 * Reads the download name from `Content-Disposition`: the UTF-8 `filename*`, else `filename`.
 *
 * Outputs: the name, or `fallback` when there is none. Side effects: none.
 */
function downloadName(disposition: string, fallback: string): string {
  const encoded = /filename\*=UTF-8''([^;]+)/i.exec(disposition)?.[1];
  if (encoded) {
    try {
      return decodeURIComponent(encoded);
    } catch {
      // Malformed: use the plain name.
    }
  }
  return /filename="([^"]+)"/.exec(disposition)?.[1] ?? fallback;
}

/**
 * Downloads a meeting's summary as a file
 * (`GET /api/meetings/{id}/summary/export?format=…`, #130).
 *
 * Outputs: the file and the name the daemon gave it (`<title> - summary.<ext>`). Throws
 * `LlmApiError` (`no_summary`, `meeting_not_found`, `export_failed`, `network`, …).
 * Side effects: none on the daemon.
 */
export async function fetchSummaryExport(meetingId: string, format: SummaryExportFormat): Promise<SummaryDownload> {
  let response: Response;
  try {
    response = await fetch(
      `${apiBaseUrl}/api/meetings/${encodeURIComponent(meetingId)}/summary/export?format=${format}`,
      { cache: "no-store" }
    );
  } catch {
    throw new LlmApiError("network", null);
  }
  if (!response.ok) {
    const body = (await response.json().catch(() => ({}))) as { error_code?: unknown };
    throw new LlmApiError(typeof body.error_code === "string" ? body.error_code : "server", response.status);
  }
  const fileName = downloadName(response.headers.get("content-disposition") ?? "", `summary.${format}`);
  return { fileName, blob: await response.blob() };
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
 * built-in one, and `paired_summary_connection: null` stops answering paired devices (#107).
 * Returns the settings as saved.
 */
export function updateLlmSettings(change: {
  default_connection?: string | null;
  template?: string | null;
  auto_summarize?: boolean;
  paired_summary_connection?: string | null;
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

/** A ready-made connection: its fields, and one line on what it is (#138). */
export type ConnectionPreset = {
  name: string;
  description?: string;
  input: Omit<LlmConnectionInput, "label" | "api_key" | "clear_key">;
};

/** Ready-made connections the Settings form can start from; online services need an API key. */
export const connectionPresets = (t: Translate): ConnectionPreset[] => [
  { name: t("localServerOllama"), description: t("presetOllama"), input: { kind: "open_ai_compatible", base_url: "http://192.168.1.10:11434/v1", model: "llama3.1:8b", context_tokens: 8192 } },
  { name: "LM Studio", description: t("presetLmStudio"), input: { kind: "open_ai_compatible", base_url: "http://192.168.1.10:1234/v1", model: "qwen3-8b", context_tokens: 32768 } },
  { name: t("claudeCodeClaudePlan"), input: { kind: "claude_code", base_url: "", model: "", context_tokens: 200000 } },
  { name: t("codexChatgptPlan"), input: { kind: "codex", base_url: "", model: "", context_tokens: 200000 } },
  { name: "Gemini", description: t("presetGemini"), input: { kind: "open_ai_compatible", base_url: "https://generativelanguage.googleapis.com/v1beta/openai", model: "gemini-2.5-flash", context_tokens: 1000000 } },
  { name: "DeepSeek", description: t("presetDeepseek"), input: { kind: "open_ai_compatible", base_url: "https://api.deepseek.com", model: "deepseek-chat", context_tokens: 128000 } },
  { name: "Qwen", description: t("presetQwen"), input: { kind: "open_ai_compatible", base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1", model: "qwen-plus", context_tokens: 131072 } },
  { name: "Kimi", description: t("presetKimi"), input: { kind: "open_ai_compatible", base_url: "https://api.moonshot.ai/v1", model: "kimi-latest", context_tokens: 131072 } },
  { name: "Mistral", description: t("presetMistral"), input: { kind: "open_ai_compatible", base_url: "https://api.mistral.ai/v1", model: "mistral-medium-latest", context_tokens: 128000 } },
  { name: "OpenRouter", description: t("presetOpenrouter"), input: { kind: "open_ai_compatible", base_url: "https://openrouter.ai/api/v1", model: "google/gemini-2.5-flash", context_tokens: 1000000 } }
];
