// Renders the voice-gallery page from the Axum API's unknown voices and saved people.

import { VoiceGallery } from "@/components/voice-gallery";
import { getVoiceGallery } from "@/lib/api";

export default async function VoiceGalleryPage() {
  const gallery = await getVoiceGallery();
  return <VoiceGallery clusters={gallery.unknown_clusters} people={gallery.identities} />;
}
