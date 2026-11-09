// Typed client for the daemon's switch that keeps online-meeting participants' voices in the voice
// gallery (#103): `GET`/`PUT /api/settings/meeting-voices`.

import { apiBaseUrl } from "@/lib/api";

/** Whether participants' voices are kept. */
export type MeetingVoices = { keep: boolean };

/** Why a request failed; `network` when the daemon was unreachable. */
export type MeetingVoicesFailureKind = "unavailable" | "server" | "network";

/** Thrown by the calls; `status` is the HTTP status, or `null` for `network`. */
export class MeetingVoicesError extends Error {
  constructor(
    readonly kind: MeetingVoicesFailureKind,
    readonly status: number | null
  ) {
    super(`Meeting voices request failed (${status ?? "network"}): ${kind}`);
    this.name = "MeetingVoicesError";
  }
}

/** Sends one request, mapping an unreachable daemon and non-2xx answers to `MeetingVoicesError`. */
async function meetingVoicesRequest(init: RequestInit): Promise<MeetingVoices> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}/api/settings/meeting-voices`, init);
  } catch {
    throw new MeetingVoicesError("network", null);
  }
  if (!response.ok) {
    throw new MeetingVoicesError(response.status === 503 ? "unavailable" : "server", response.status);
  }
  return (await response.json()) as MeetingVoices;
}

/**
 * Reads whether participants' voices are kept (`GET /api/settings/meeting-voices`).
 *
 * Outputs: the choice. Throws `MeetingVoicesError`. Side effects: none.
 */
export async function getMeetingVoices(): Promise<MeetingVoices> {
  return meetingVoicesRequest({ cache: "no-store" });
}

/**
 * Switches keeping participants' voices (`PUT /api/settings/meeting-voices`).
 *
 * Inputs: the choice. Outputs: the choice as saved. Throws `MeetingVoicesError`.
 * Side effects: rewrites the daemon's transcription settings; applies at the next offer.
 */
export async function setMeetingVoices(keep: boolean): Promise<MeetingVoices> {
  return meetingVoicesRequest({
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ keep })
  });
}
