// Defines typed REST and WebSocket clients for the documented local Axum API.

export type Device = { device_id: string; supports_vad: boolean; supported_models: string[]; last_seen: number };
export type Metric = { task_type: "transcription" | "cloud_transcription" | "file_transfer" | "diarization"; duration_ms: number; latency_ms: number; power_mwh: number; recorded_at: number };
export type DeviceTelemetry = { device_id: string; metrics: Metric[] };
export type TelemetrySummary = { total_transcription_time_ms: number; average_latency_ms: number; total_tasks: number; power_estimate_mwh: number };
export type MeetingSummary = { meeting_id: string; title: string; created_at: string };
export type Attachment = { attachment_id: string; file_path: string; mime_type: string; original_timestamp_utc: string };
export type MeetingDetail = { meeting_id: string; title: string; created_at: string; transcript: string | null; attachments: Attachment[] };
export type VoiceIdentity = { identity_id: string; display_name: string; voice_variant_count: number };
export type UnassignedClip = { clip_id: string; meeting_id: string; start_time_ms: number; end_time_ms: number; audio_url?: string };
export type UnknownCluster = { cluster_id: string; meeting_id: string; total_duration_ms: number; chunk_count: number; created_at: string };
/** `voice_group_id` is shared by voices that sound like one person (#82); older servers omit it. */
export type LibraryCluster = UnknownCluster & { meeting_title: string | null; has_review_clip: boolean; voice_group_id?: string };
export type VoiceGalleryResponse = { identities: VoiceIdentity[]; unknown_clusters: LibraryCluster[]; unassigned_clips: UnassignedClip[] };
export type VoiceGroupAssignment = { identity_id: string; identity_name: string; identity_created: boolean; assigned_cluster_ids: string[]; skipped_cluster_count: number; variants_stored: number; segments_updated: number };
export type ClusterAssignment = { cluster_id: string; identity_id: string; identity_name: string; identity_created: boolean; variants_stored: number; segments_updated: number };

export const apiBaseUrl = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://127.0.0.1:8080";

async function apiGet<T>(path: string): Promise<T> {
  const response = await fetch(`${apiBaseUrl}${path}`, { cache: "no-store" });
  if (!response.ok) throw new Error(`Axum API request failed: ${response.status}`);
  return response.json() as Promise<T>;
}

export const getDevices = () => apiGet<{ devices: Device[] }>("/api/devices");
export const getDeviceTelemetry = (deviceId: string) => apiGet<DeviceTelemetry>(`/api/telemetry/devices/${encodeURIComponent(deviceId)}`);
export const getTelemetrySummary = () => apiGet<TelemetrySummary>("/api/telemetry/summary");
export const getMeetings = () => apiGet<MeetingSummary[]>("/api/meetings");
export const getMeeting = (meetingId: string) => apiGet<MeetingDetail>(`/api/meetings/${encodeURIComponent(meetingId)}`);
export const reviewClipUrl = (clusterId: string) => `${apiBaseUrl}/api/clusters/${encodeURIComponent(clusterId)}/review-clip`;
export const getVoiceGallery = () => apiGet<VoiceGalleryResponse>("/api/voice-gallery");
export const getUnknownClusters = (meetingId: string) => apiGet<UnknownCluster[]>(`/api/meetings/${encodeURIComponent(meetingId)}/unknown-clusters`);

/** Assigns an unknown-speaker cluster to a named identity; throws with the server's message on failure. */
export async function assignCluster(clusterId: string, identityName: string): Promise<ClusterAssignment> {
  const response = await fetch(`${apiBaseUrl}/api/clusters/${encodeURIComponent(clusterId)}/assign`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ identity_name: identityName })
  });
  if (!response.ok) {
    const errorBody = (await response.json().catch(() => ({}))) as { error?: string };
    throw new Error(`Cluster assignment failed (${response.status}): ${errorBody.error ?? "unknown error"}`);
  }
  return response.json() as Promise<ClusterAssignment>;
}

/**
 * Names several unknown voices as one person (#82); voices named or gone meanwhile are skipped.
 * Throws with the server's message on failure.
 */
export async function assignVoiceGroup(clusterIds: string[], identityName: string): Promise<VoiceGroupAssignment> {
  const response = await fetch(`${apiBaseUrl}/api/voice-groups/assign`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ cluster_ids: clusterIds, identity_name: identityName })
  });
  if (!response.ok) {
    const errorBody = (await response.json().catch(() => ({}))) as { error?: string };
    throw new Error(`Voice group assignment failed (${response.status}): ${errorBody.error ?? "unknown error"}`);
  }
  return response.json() as Promise<VoiceGroupAssignment>;
}

/** Why `renameMeeting` failed: the HTTP status class, or `network` when the daemon was unreachable. */
export type MeetingRenameFailureKind = "invalid_title" | "not_found" | "server" | "network" | "unexpected";

/** Thrown by `renameMeeting`; `serverMessage` is for diagnostics and is not meant to be shown. */
export class MeetingRenameError extends Error {
  constructor(
    readonly kind: MeetingRenameFailureKind,
    readonly status: number | null,
    readonly serverMessage: string | null
  ) {
    super(`Meeting rename failed (${status ?? "network"}): ${kind}`);
    this.name = "MeetingRenameError";
  }
}

/** Maps a non-2xx `PATCH /api/meetings/{id}` status to its failure kind. */
function renameFailureKind(status: number): MeetingRenameFailureKind {
  if (status === 400) return "invalid_title";
  if (status === 404) return "not_found";
  if (status >= 500) return "server";
  return "unexpected";
}

/**
 * Renames a meeting through `PATCH /api/meetings/{id}` with `{ "title": newTitle }`.
 *
 * Inputs: the opaque meeting ID (URL-encoded here) and the new title; the server trims it and
 * requires 1–200 characters without control characters.
 * Outputs: the updated meeting summary (200). Throws `MeetingRenameError` with kind
 * `invalid_title` (400), `not_found` (404), `server` (5xx), `unexpected` (any other status), or
 * `network` (the request never completed).
 * Side effects: writes the title to the meeting's CRDT document and SQLite on the daemon.
 */
export async function renameMeeting(meetingId: string, newTitle: string): Promise<MeetingSummary> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}/api/meetings/${encodeURIComponent(meetingId)}`, {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ title: newTitle })
    });
  } catch {
    throw new MeetingRenameError("network", null, null);
  }
  if (!response.ok) {
    const errorBody = (await response.json().catch(() => ({}))) as { error?: unknown };
    const serverMessage = typeof errorBody.error === "string" ? errorBody.error : null;
    throw new MeetingRenameError(renameFailureKind(response.status), response.status, serverMessage);
  }
  return response.json() as Promise<MeetingSummary>;
}

export async function assignVoiceClip(clipId: string, identityId: string): Promise<void> {
  const response = await fetch(`${apiBaseUrl}/api/voice/assign`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ clip_id: clipId, identity_id: identityId })
  });
  if (!response.ok) throw new Error(`Voice assignment failed: ${response.status}`);
}

/** Why a log settings request failed; `network` when the daemon was unreachable (Issue #46). */
export type LogSettingsFailureKind = "invalid_level" | "no_logs" | "unavailable" | "server" | "network";

/** Thrown by the log settings calls; `status` is the HTTP status, or `null` for `network`. */
export class LogSettingsError extends Error {
  constructor(
    readonly kind: LogSettingsFailureKind,
    readonly status: number | null
  ) {
    super(`Log settings request failed (${status ?? "network"}): ${kind}`);
    this.name = "LogSettingsError";
  }
}

/** The logs as downloaded: the file and the name the daemon gave it. */
export type LogDownload = { fileName: string; blob: Blob };

/** Sends one log settings request, mapping an unreachable daemon and non-2xx answers to `LogSettingsError`. */
async function logSettingsRequest(path: string, init: RequestInit): Promise<Response> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, init);
  } catch {
    throw new LogSettingsError("network", null);
  }
  if (!response.ok) {
    const errorBody = (await response.json().catch(() => ({}))) as { error_code?: unknown };
    const kind: LogSettingsFailureKind =
      errorBody.error_code === "invalid_level" || errorBody.error_code === "no_logs"
        ? errorBody.error_code
        : response.status === 503
          ? "unavailable"
          : "server";
    throw new LogSettingsError(kind, response.status);
  }
  return response;
}

/**
 * Reads the daemon's log level through `GET /api/settings/log-level`.
 *
 * Outputs: the `tracing` directive in effect, such as `info`. Throws `LogSettingsError`.
 * Side effects: none.
 */
export async function getLogLevel(): Promise<string> {
  const response = await logSettingsRequest("/api/settings/log-level", { cache: "no-store" });
  return ((await response.json()) as { level: string }).level;
}

/**
 * Changes the daemon's log level through `PUT /api/settings/log-level` until it restarts.
 *
 * Inputs: a directive such as `debug`. Outputs: the level now in effect. Throws `LogSettingsError`
 * (`invalid_level` for a directive the daemon refuses).
 * Side effects: reloads the daemon's logging filter.
 */
export async function setLogLevel(level: string): Promise<string> {
  const response = await logSettingsRequest("/api/settings/log-level", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ level })
  });
  return ((await response.json()) as { level: string }).level;
}

/**
 * Downloads the newest week of the daemon's logs through `GET /api/logs/export`.
 *
 * Outputs: the JSON-lines file and its name. Throws `LogSettingsError` (`no_logs` when there are none).
 * Side effects: none on the daemon beyond a temporary file it removes.
 */
export async function fetchLogExport(): Promise<LogDownload> {
  const response = await logSettingsRequest("/api/logs/export", { cache: "no-store" });
  const disposition = response.headers.get("content-disposition") ?? "";
  const fileName = /filename="([^"]+)"/.exec(disposition)?.[1] ?? "note_taker_logs.jsonl";
  return { fileName, blob: await response.blob() };
}

export function webSocketUrl(): string {
  if (process.env.NEXT_PUBLIC_WS_URL) return process.env.NEXT_PUBLIC_WS_URL;
  return `${apiBaseUrl.replace(/^http/, "ws")}/ws`;
}

/** Why a deletion failed: `recording` when the meeting is being recorded, else any other refusal. */
export class DeletionError extends Error {
  constructor(readonly kind: "recording" | "failed") {
    super(`Deletion failed: ${kind}`);
    this.name = "DeletionError";
  }
}

/**
 * Sends a `DELETE` to `path`. An item already gone (404) counts as deleted.
 *
 * Inputs: the API path. Outputs: resolves when the item is gone; throws `DeletionError` otherwise.
 * Side effects: irreversibly deletes the item on the daemon and, through sync, on paired devices.
 */
async function deleteItem(path: string): Promise<void> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, { method: "DELETE" });
  } catch {
    throw new DeletionError("failed");
  }
  if (response.status === 409) throw new DeletionError("recording");
  if (!response.ok && response.status !== 404) throw new DeletionError("failed");
}

/** Deletes a meeting with its transcript, attachments and unknown voices. See `deleteItem`. */
export function deleteMeeting(meetingId: string): Promise<void> {
  return deleteItem(`/api/meetings/${encodeURIComponent(meetingId)}`);
}

/** Deletes an unknown voice; its transcript stays. See `deleteItem`. */
export function deleteUnknownVoice(clusterId: string): Promise<void> {
  return deleteItem(`/api/clusters/${encodeURIComponent(clusterId)}`);
}

/** Deletes a saved person's name and voice samples. See `deleteItem`. */
export function deleteSavedPerson(identityId: string): Promise<void> {
  return deleteItem(`/api/identities/${encodeURIComponent(identityId)}`);
}
