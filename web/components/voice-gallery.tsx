// Lists unknown voices, assigns each one to a new or saved person through the Axum API, and lists
// the saved people (Issue #63).

"use client";

import { FormEvent, useState } from "react";
import { ConfirmDeleteDialog } from "@/components/confirm-delete-dialog";
import { DeletionError, assignCluster, deleteSavedPerson, deleteUnknownVoice, reviewClipUrl, type LibraryCluster, type VoiceIdentity } from "@/lib/api";

/** A deletion waiting for the owner's answer. */
type PendingDeletion = { title: string; message: string; run: () => Promise<void> };

/** A plain message for a failed voice deletion; server text is not shown. */
function deletionFailureMessage(error: unknown): string {
  return error instanceof DeletionError && error.kind === "recording"
    ? "This voice's meeting is being recorded. Stop the recording first."
    : "Couldn't delete this. Check that the note-taking server is running.";
}

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
  const [pendingDeletion, setPendingDeletion] = useState<PendingDeletion | null>(null);

  /** Asks before deleting an unknown voice, then removes its card. */
  function askToDeleteVoice(cluster: LibraryCluster, speakerLabel: string) {
    setPendingDeletion({
      title: `Delete ${speakerLabel}?`,
      message: "Its voice sample goes for good, here and on your paired devices. The transcript stays.",
      run: async () => {
        await deleteUnknownVoice(cluster.cluster_id);
        setPendingClusters((current) => current.filter((candidate) => candidate.cluster_id !== cluster.cluster_id));
        setMessage("Voice deleted.");
      }
    });
  }

  /** Asks before deleting a saved person, then removes them from the list. */
  function askToDeletePerson(person: VoiceIdentity) {
    setPendingDeletion({
      title: `Delete ${person.display_name}?`,
      message: "Their name and voice samples go for good, here and on your paired devices. Transcripts stay and show an unknown speaker.",
      run: async () => {
        await deleteSavedPerson(person.identity_id);
        setPeople((current) => current.filter((candidate) => candidate.identity_id !== person.identity_id));
        setMessage(`${person.display_name} deleted.`);
      }
    });
  }

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
    <section className="space-y-4">
      <h1 className="text-3xl font-bold">Voice gallery</h1>
      <datalist id="saved-people">
        {people.map((person) => <option key={person.identity_id} value={person.display_name} />)}
      </datalist>
      <h2 className="text-lg font-semibold">Unknown speakers</h2>
      {pendingClusters.length === 0 ? <p>No unknown speakers to review.</p> : null}
      {pendingClusters.map((cluster, index) => {
        const speakerLabel = `Unknown speaker ${index + 1}`;
        return (
          <article className="card space-y-2" key={cluster.cluster_id}>
            <div className="flex items-center justify-between gap-2">
              <h3 className="font-semibold">{speakerLabel}</h3>
              <button
                type="button"
                aria-label={`Delete ${speakerLabel}`}
                onClick={() => askToDeleteVoice(cluster, speakerLabel)}
                className="min-h-11 rounded-lg px-3 font-medium text-danger hover:bg-surface-muted"
              >
                Delete
              </button>
            </div>
            <p>{formatSummary(cluster)}</p>
            <p>{cluster.meeting_title ?? "Untitled meeting"}</p>
            {cluster.has_review_clip ? <audio controls preload="none" src={reviewClipUrl(cluster.cluster_id)} aria-label={`Listen to ${speakerLabel.toLowerCase()}`} /> : null}
            <form className="space-y-2" onSubmit={(event) => submitAssignment(event, cluster)}>
              <label className="block">Name
                <input className="mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30" name="identityName" type="text" aria-label={`Name for ${speakerLabel.toLowerCase()}`} autoComplete="off" list="saved-people" />
              </label>
              <button className="min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong" type="submit">Assign</button>
            </form>
          </article>
        );
      })}
      <p aria-live="polite">{message}</p>
      <h2 className="text-lg font-semibold">Saved people</h2>
      {people.length === 0 ? <p>No saved people yet. Name a voice to add someone.</p> : null}
      <ul>
        {people.map((person) => (
          <li key={person.identity_id} className="flex items-center justify-between gap-2">
            <span>{person.display_name}: {person.voice_variant_count} voice {person.voice_variant_count === 1 ? "sample" : "samples"}</span>
            <button
              type="button"
              aria-label={`Delete ${person.display_name}`}
              onClick={() => askToDeletePerson(person)}
              className="min-h-11 rounded-lg px-3 font-medium text-danger hover:bg-surface-muted"
            >
              Delete
            </button>
          </li>
        ))}
      </ul>
      {pendingDeletion ? (
        <ConfirmDeleteDialog
          title={pendingDeletion.title}
          message={pendingDeletion.message}
          onClose={() => setPendingDeletion(null)}
          onConfirm={async () => {
            try {
              await pendingDeletion.run();
            } catch (error) {
              throw new Error(deletionFailureMessage(error));
            }
            setPendingDeletion(null);
          }}
        />
      ) : null}
    </section>
  );
}
