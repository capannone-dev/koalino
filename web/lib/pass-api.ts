// Typed client for the daemon's after-meeting pass (#87): whether each meeting is transcribed again
// after Stop, and with which model. `GET`/`PUT /api/settings/after-meeting-pass`.

import { apiBaseUrl } from "@/lib/api";

/** One Whisper model on the server's machine. */
export type PassModel = { file_name: string; name: string };

/** The pass's choices: on or off, its model (`null` = the recording model), and the models here. */
export type AfterMeetingPass = { enabled: boolean; model: string | null; models: PassModel[] };

/** Why a request failed; `network` when the daemon was unreachable. */
export type PassFailureKind = "invalid_model" | "unavailable" | "server" | "network";

/** Thrown by the calls; `status` is the HTTP status, or `null` for `network`. */
export class PassError extends Error {
  constructor(
    readonly kind: PassFailureKind,
    readonly status: number | null
  ) {
    super(`After-meeting pass request failed (${status ?? "network"}): ${kind}`);
    this.name = "PassError";
  }
}

/** Sends one request, mapping an unreachable daemon and non-2xx answers to `PassError`. */
async function passRequest(init: RequestInit): Promise<AfterMeetingPass> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}/api/settings/after-meeting-pass`, init);
  } catch {
    throw new PassError("network", null);
  }
  if (!response.ok) {
    const errorBody = (await response.json().catch(() => ({}))) as { error_code?: unknown };
    const kind: PassFailureKind =
      errorBody.error_code === "invalid_model" ? "invalid_model" : response.status === 503 ? "unavailable" : "server";
    throw new PassError(kind, response.status);
  }
  return (await response.json()) as AfterMeetingPass;
}

/**
 * Reads the pass's choices (`GET /api/settings/after-meeting-pass`).
 *
 * Outputs: the choices and the models on the server's machine. Throws `PassError`. Side effects: none.
 */
export async function getAfterMeetingPass(): Promise<AfterMeetingPass> {
  return passRequest({ cache: "no-store" });
}

/**
 * Changes the pass's choices (`PUT /api/settings/after-meeting-pass`).
 *
 * Inputs: `enabled` and/or `model` (`null` for the recording model); a missing field is kept.
 * Outputs: the choices as saved. Throws `PassError` (`invalid_model` for a model not on that
 * machine). Side effects: rewrites the daemon's transcription settings; the next recording uses them.
 */
export async function setAfterMeetingPass(change: { enabled?: boolean; model?: string | null }): Promise<AfterMeetingPass> {
  return passRequest({
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(change)
  });
}
