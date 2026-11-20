// Typed client for the owner's kept meeting recordings (#140): the per-device switch, what is
// kept, wiping them all, and a meeting's recording as WAV.

import { apiBaseUrl } from "@/lib/api";

/** The switch and what is kept, as the daemon describes them. */
export type RecordingsStatus = { keep: boolean; count: number; bytes: number };

/** Thrown when a recordings request fails; `code` is the daemon's `error_code`, or `network`. */
export class RecordingsError extends Error {
  constructor(readonly code: string) {
    super(`Recordings request failed: ${code}`);
    this.name = "RecordingsError";
  }
}

/** Sends one request and reads its JSON, mapping failures to `RecordingsError`. */
async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, { cache: "no-store", ...init });
  } catch {
    throw new RecordingsError("network");
  }
  if (!response.ok) {
    const body = (await response.json().catch(() => ({}))) as { error_code?: unknown };
    throw new RecordingsError(typeof body.error_code === "string" ? body.error_code : "server");
  }
  return (await response.json()) as T;
}

/** Reads the switch and what is kept (`GET /api/settings/recordings`). */
export function getRecordingsStatus(): Promise<RecordingsStatus> {
  return request<RecordingsStatus>("/api/settings/recordings");
}

/** Turns keeping recordings on or off (`PUT /api/settings/recordings`); answers the new status. */
export function setKeepRecordings(keep: boolean): Promise<RecordingsStatus> {
  return request<RecordingsStatus>("/api/settings/recordings", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ keep })
  });
}

/** Deletes every kept recording (`DELETE /api/recordings`); answers how many were deleted. */
export async function wipeRecordings(): Promise<number> {
  const answer = await request<{ deleted: number }>("/api/recordings", { method: "DELETE" });
  return answer.deleted;
}

/** Whether a meeting has a kept recording, without decrypting it. */
export async function hasMeetingRecording(meetingId: string): Promise<boolean> {
  const answer = await request<{ available: boolean }>(
    `/api/meetings/${encodeURIComponent(meetingId)}/recording/available`
  );
  return answer.available === true;
}

/** The meeting's recording as WAV, for an `<audio>` element. */
export const meetingRecordingUrl = (meetingId: string) =>
  `${apiBaseUrl}/api/meetings/${encodeURIComponent(meetingId)}/recording`;

/** Words a size in bytes as MB with one decimal under 10 MB. */
export function recordingsSize(bytes: number): string {
  const megabytes = bytes / (1024 * 1024);
  return megabytes.toFixed(megabytes < 10 ? 1 : 0);
}
