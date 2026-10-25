// Defines typed REST and WebSocket clients for the documented local Axum API.

export type Telemetry = { task_count: number; total_duration_ms: number; average_latency_ms: number };
export type MeetingSummary = { meeting_id: string };
export type Attachment = { attachment_id: string; file_path: string; mime_type: string; original_timestamp_utc: string };
export type MeetingDetail = { meeting_id: string; transcript: string | null; attachments: Attachment[] };
export type VoiceIdentity = { identity_id: string; display_name: string };
export type UnassignedClip = { clip_id: string; meeting_id: string; start_time_ms: number; end_time_ms: number; audio_url?: string };
export type VoiceGalleryResponse = { identities: VoiceIdentity[]; unassigned_clips: UnassignedClip[] };

const apiBaseUrl = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://127.0.0.1:8080";

async function apiGet<T>(path: string): Promise<T> {
  const response = await fetch(`${apiBaseUrl}${path}`, { cache: "no-store" });
  if (!response.ok) throw new Error(`Axum API request failed: ${response.status}`);
  return response.json() as Promise<T>;
}

export const getTelemetry = () => apiGet<Telemetry>("/api/telemetry");
export const getMeetings = () => apiGet<MeetingSummary[]>("/api/meetings");
export const getMeeting = (meetingId: string) => apiGet<MeetingDetail>(`/api/meetings/${encodeURIComponent(meetingId)}`);
export const getVoiceGallery = () => apiGet<VoiceGalleryResponse>("/api/voice-gallery");

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
