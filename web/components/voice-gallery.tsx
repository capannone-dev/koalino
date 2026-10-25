// Provides local voice-gallery presentation and explicit assignment posting to Axum.

"use client";

import { FormEvent, useState } from "react";
import { assignVoiceClip, type VoiceGalleryResponse } from "@/lib/api";

export function VoiceGallery({ gallery }: { gallery: VoiceGalleryResponse }) {
  const [selectedIdentityId, setSelectedIdentityId] = useState(gallery.identities[0]?.identity_id ?? "");
  const [message, setMessage] = useState("");

  async function submitAssignment(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const formData = new FormData(event.currentTarget);
    const clipId = String(formData.get("clipId") ?? "");
    try {
      await assignVoiceClip(clipId, selectedIdentityId);
      setMessage("Voice clip assigned.");
    } catch {
      setMessage("Unable to assign the voice clip.");
    }
  }

  return (
    <section>
      <h1>Voice gallery</h1>
      <h2>Known identities</h2>
      <ul>{gallery.identities.map((identity) => <li key={identity.identity_id}>{identity.display_name}</li>)}</ul>
      <h2>Unassigned clips</h2>
      {gallery.unassigned_clips.map((clip) => (
        <article className="card" key={clip.clip_id}>
          <p>Clip {clip.clip_id} ({clip.start_time_ms}–{clip.end_time_ms} ms)</p>
          {clip.audio_url ? <audio controls src={clip.audio_url}>Audio unavailable.</audio> : <p>Audio playback is unavailable until the local server supplies a review-media URL.</p>}
          <form onSubmit={submitAssignment}>
            <input name="clipId" type="hidden" value={clip.clip_id} />
            <label>Assign identity
              <select aria-label="Assign identity" value={selectedIdentityId} onChange={(event) => setSelectedIdentityId(event.target.value)} required>
                <option value="" disabled>Select an identity</option>
                {gallery.identities.map((identity) => <option key={identity.identity_id} value={identity.identity_id}>{identity.display_name}</option>)}
              </select>
            </label>
            <button type="submit" disabled={!selectedIdentityId}>Assign clip</button>
          </form>
        </article>
      ))}
      <p aria-live="polite">{message}</p>
    </section>
  );
}
