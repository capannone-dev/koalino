// Lists unknown-speaker clusters and assigns each one to a typed name through the Axum API.

"use client";

import { FormEvent, useState } from "react";
import { assignCluster, type UnknownCluster } from "@/lib/api";

/** Formats total speech time (seconds, one decimal) and chunk count, e.g. `12.5 s · 3 chunks`. */
function formatSummary(cluster: UnknownCluster): string {
  const tenthsOfSecond = Math.floor((cluster.total_duration_ms + 50) / 100);
  const chunkLabel = cluster.chunk_count === 1 ? "chunk" : "chunks";
  return `${Math.floor(tenthsOfSecond / 10)}.${tenthsOfSecond % 10} s · ${cluster.chunk_count} ${chunkLabel}`;
}

export function VoiceGallery({ clusters }: { clusters: UnknownCluster[] }) {
  const [pendingClusters, setPendingClusters] = useState(clusters);
  const [message, setMessage] = useState("");

  async function submitAssignment(event: FormEvent<HTMLFormElement>, cluster: UnknownCluster) {
    event.preventDefault();
    const identityName = String(new FormData(event.currentTarget).get("identityName") ?? "").trim();
    if (!identityName) {
      setMessage("Enter a name before assigning.");
      return;
    }
    try {
      const assignment = await assignCluster(cluster.cluster_id, identityName);
      setPendingClusters((current) => current.filter((candidate) => candidate.cluster_id !== cluster.cluster_id));
      setMessage(`Assigned to ${assignment.identity_name}.`);
    } catch {
      setMessage("Unable to assign the speaker cluster.");
    }
  }

  return (
    <section>
      <h1>Voice gallery</h1>
      <h2>Unknown speakers</h2>
      {pendingClusters.length === 0 ? <p>No unknown speakers to review.</p> : null}
      {pendingClusters.map((cluster, index) => {
        const speakerLabel = `Unknown speaker ${index + 1}`;
        return (
          <article className="card" key={cluster.cluster_id}>
            <h3>{speakerLabel}</h3>
            <p>{formatSummary(cluster)}</p>
            <p>Meeting {cluster.meeting_id}</p>
            <form onSubmit={(event) => submitAssignment(event, cluster)}>
              <label>Name
                <input name="identityName" type="text" aria-label={`Name for ${speakerLabel.toLowerCase()}`} autoComplete="off" />
              </label>
              <button type="submit">Assign</button>
            </form>
          </article>
        );
      })}
      <p aria-live="polite">{message}</p>
    </section>
  );
}
