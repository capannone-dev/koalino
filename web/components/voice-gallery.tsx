// Lists unknown voices, assigns each one to a new or saved person through the Axum API, and lists
// the saved people (Issue #63).

"use client";

import { FormEvent, useState } from "react";
import { assignCluster, reviewClipUrl, type LibraryCluster, type VoiceIdentity } from "@/lib/api";

/** Formats total speech time (seconds, one decimal) and chunk count, e.g. `12.5 s · 3 chunks`. */
function formatSummary(cluster: LibraryCluster): string {
  const tenthsOfSecond = Math.floor((cluster.total_duration_ms + 50) / 100);
  const chunkLabel = cluster.chunk_count === 1 ? "chunk" : "chunks";
  return `${Math.floor(tenthsOfSecond / 10)}.${tenthsOfSecond % 10} s · ${cluster.chunk_count} ${chunkLabel}`;
}

export function VoiceGallery({ clusters, people: initialPeople }: { clusters: LibraryCluster[]; people: VoiceIdentity[] }) {
  const [pendingClusters, setPendingClusters] = useState(clusters);
  const [people, setPeople] = useState(initialPeople);
  const [message, setMessage] = useState("");

  async function submitAssignment(event: FormEvent<HTMLFormElement>, cluster: LibraryCluster) {
    event.preventDefault();
    const identityName = String(new FormData(event.currentTarget).get("identityName") ?? "").trim();
    if (!identityName) {
      setMessage("Enter a name before assigning.");
      return;
    }
    try {
      const assignment = await assignCluster(cluster.cluster_id, identityName);
      setPendingClusters((current) => current.filter((candidate) => candidate.cluster_id !== cluster.cluster_id));
      setPeople((current) => [
        ...current.filter((person) => person.identity_id !== assignment.identity_id),
        {
          identity_id: assignment.identity_id,
          display_name: assignment.identity_name,
          voice_variant_count: (current.find((person) => person.identity_id === assignment.identity_id)?.voice_variant_count ?? 0) + assignment.variants_stored
        }
      ].sort((first, second) => first.display_name.localeCompare(second.display_name)));
      setMessage(`Assigned to ${assignment.identity_name}.`);
    } catch {
      setMessage("Unable to assign the speaker cluster.");
    }
  }

  return (
    <section>
      <h1>Voice gallery</h1>
      <datalist id="saved-people">
        {people.map((person) => <option key={person.identity_id} value={person.display_name} />)}
      </datalist>
      <h2>Unknown speakers</h2>
      {pendingClusters.length === 0 ? <p>No unknown speakers to review.</p> : null}
      {pendingClusters.map((cluster, index) => {
        const speakerLabel = `Unknown speaker ${index + 1}`;
        return (
          <article className="card" key={cluster.cluster_id}>
            <h3>{speakerLabel}</h3>
            <p>{formatSummary(cluster)}</p>
            <p>{cluster.meeting_title ?? "Untitled meeting"}</p>
            {cluster.has_review_clip ? <audio controls preload="none" src={reviewClipUrl(cluster.cluster_id)} aria-label={`Listen to ${speakerLabel.toLowerCase()}`} /> : null}
            <form onSubmit={(event) => submitAssignment(event, cluster)}>
              <label>Name
                <input name="identityName" type="text" aria-label={`Name for ${speakerLabel.toLowerCase()}`} autoComplete="off" list="saved-people" />
              </label>
              <button type="submit">Assign</button>
            </form>
          </article>
        );
      })}
      <p aria-live="polite">{message}</p>
      <h2>Saved people</h2>
      {people.length === 0 ? <p>No saved people yet. Name a voice to add someone.</p> : null}
      <ul>
        {people.map((person) => (
          <li key={person.identity_id}>
            {person.display_name}: {person.voice_variant_count} voice {person.voice_variant_count === 1 ? "sample" : "samples"}
          </li>
        ))}
      </ul>
    </section>
  );
}
