// Defines typed REST and WebSocket clients for the documented local Axum API.

export type Telemetry = { task_count: number; total_duration_ms: number; average_latency_ms: number };
export type Device = { device_id: string; supports_vad: boolean; supported_models: string[]; last_seen: number };
export type Metric = { task_type: "transcription" | "cloud_transcription" | "file_transfer" | "diarization"; duration_ms: number; latency_ms: number; power_mwh: number; recorded_at: number };
export type DeviceTelemetry = { device_id: string; metrics: Metric[] };
export type TelemetrySummary = { total_transcription_time_ms: number; average_latency_ms: number; total_tasks: number; power_estimate_mwh: number };
export type MeetingSummary = { meeting_id: string; title: string; created_at: string };
export type Attachment = { attachment_id: string; file_path: string; mime_type: string; original_timestamp_utc: string };
export type MeetingDetail = { meeting_id: string; title: string; created_at: string; transcript: string | null; attachments: Attachment[] };
export type VoiceIdentity = { identity_id: string; display_name: string };
export type UnassignedClip = { clip_id: string; meeting_id: string; start_time_ms: number; end_time_ms: number; audio_url?: string };
export type VoiceGalleryResponse = { identities: VoiceIdentity[]; unassigned_clips: UnassignedClip[] };
export type UnknownCluster = { cluster_id: string; meeting_id: string; total_duration_ms: number; chunk_count: number; created_at: string };
export type ClusterAssignment = { cluster_id: string; identity_id: string; identity_name: string; identity_created: boolean; variants_stored: number; segments_updated: number };

const apiBaseUrl = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://127.0.0.1:8080";

async function apiGet<T>(path: string): Promise<T> {
  const response = await fetch(`${apiBaseUrl}${path}`, { cache: "no-store" });
  if (!response.ok) throw new Error(`Axum API request failed: ${response.status}`);
  return response.json() as Promise<T>;
}

export const getTelemetry = () => apiGet<Telemetry>("/api/telemetry");
export const getDevices = () => apiGet<{ devices: Device[] }>("/api/devices");
export const getDeviceTelemetry = (deviceId: string) => apiGet<DeviceTelemetry>(`/api/telemetry/devices/${encodeURIComponent(deviceId)}`);
export const getTelemetrySummary = () => apiGet<TelemetrySummary>("/api/telemetry/summary");
export const getMeetings = () => apiGet<MeetingSummary[]>("/api/meetings");
export const getMeeting = (meetingId: string) => apiGet<MeetingDetail>(`/api/meetings/${encodeURIComponent(meetingId)}`);
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

export async function assignVoiceClip(clipId: string, identityId: string): Promise<void> {
  const response = await fetch(`${apiBaseUrl}/api/voice/assign`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ clip_id: clipId, identity_id: identityId })
  });
  if (!response.ok) throw new Error(`Voice assignment failed: ${response.status}`);
}

export function webSocketUrl(): string {
  if (process.env.NEXT_PUBLIC_WS_URL) return process.env.NEXT_PUBLIC_WS_URL;
  return `${apiBaseUrl.replace(/^http/, "ws")}/ws`;
}
