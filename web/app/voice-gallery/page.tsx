// Renders the voice-gallery management page from Axum data and a client assignment form.

import { VoiceGallery } from "@/components/voice-gallery";
import { getVoiceGallery } from "@/lib/api";

export default async function VoiceGalleryPage() {
  const gallery = await getVoiceGallery();
  return <VoiceGallery gallery={gallery} />;
}
