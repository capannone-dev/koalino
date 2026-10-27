// Renders the voice-gallery page from every meeting's unknown-speaker clusters on the Axum API.

import { VoiceGallery } from "@/components/voice-gallery";
import { getMeetings, getUnknownClusters } from "@/lib/api";

export default async function VoiceGalleryPage() {
  const meetings = await getMeetings();
  const clusters = (await Promise.all(meetings.map((meeting) => getUnknownClusters(meeting.meeting_id)))).flat();
  return <VoiceGallery clusters={clusters} />;
}
