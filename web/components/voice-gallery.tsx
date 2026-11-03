// Lists unknown voices, assigns each one to a new or saved person through the Axum API, and lists
// the saved people (Issue #63). Voices that sound like one person share a card and are named at
// once; the owner can leave out a part that is someone else (#82).

"use client";

import { FormEvent, useState } from "react";
import { ConfirmDeleteDialog } from "@/components/confirm-delete-dialog";
import { DeletionError, assignCluster, assignVoiceGroup, deleteSavedPerson, deleteUnknownVoice, reviewClipUrl, type LibraryCluster, type VoiceIdentity } from "@/lib/api";

/** A deletion waiting for the owner's answer. */
type PendingDeletion = { title: string; message: string; run: () => Promise<void> };

/** Voices of one person, in listing order (#82). */
type VoiceGroup = { groupId: string; voices: LibraryCluster[] };

/** A plain message for a failed voice deletion; server text is not shown. */
function deletionFailureMessage(error: unknown): string {
  return error instanceof DeletionError && error.kind === "recording"
    ? "This voice's meeting is being recorded. Stop the recording first."
    : "Couldn't delete this. Check that the note-taking server is running.";
}

/** Formats milliseconds as seconds with one decimal, e.g. `12.5 s`. */
function formatSeconds(milliseconds: number): string {
  const tenthsOfSecond = Math.floor((milliseconds + 50) / 100);
  return `${Math.floor(tenthsOfSecond / 10)}.${tenthsOfSecond % 10} s`;
}

/** Formats total speech time (seconds, one decimal) and chunk count, e.g. `12.5 s · 3 chunks`. */
function formatSummary(cluster: LibraryCluster): string {
  const chunkLabel = cluster.chunk_count === 1 ? "chunk" : "chunks";
  return `${formatSeconds(cluster.total_duration_ms)} · ${cluster.chunk_count} ${chunkLabel}`;
}

/**
 * Collects voices into groups by `voice_group_id` (a voice without one is its own group), keeping
 * the order of each group's first voice. Side effects: none.
 */
function groupVoices(clusters: LibraryCluster[]): VoiceGroup[] {
  const groups = new Map<string, LibraryCluster[]>();
  for (const cluster of clusters) {
    const groupId = cluster.voice_group_id ?? cluster.cluster_id;
    groups.set(groupId, [...(groups.get(groupId) ?? []), cluster]);
  }
  return [...groups].map(([groupId, voices]) => ({ groupId, voices }));
}

/** Says what a group covers, e.g. `134.0 s · 2 parts in 2 meetings`. */
function formatGroupSummary(group: VoiceGroup): string {
  const totalMs = group.voices.reduce((sum, voice) => sum + voice.total_duration_ms, 0);
  const meetingCount = new Set(group.voices.map((voice) => voice.meeting_id)).size;
  const parts = `${group.voices.length} parts`;
  return `${formatSeconds(totalMs)} · ${meetingCount === 1 ? parts : `${parts} in ${meetingCount} meetings`}`;
}

/** Labels one voice of a group by meeting and length, e.g. `Lunch · 105.0 s`. */
function partLabel(voice: LibraryCluster): string {
  return `${voice.meeting_title ?? "Untitled meeting"} · ${formatSeconds(voice.total_duration_ms)}`;
}

export function VoiceGallery({ clusters, people: initialPeople }: { clusters: LibraryCluster[]; people: VoiceIdentity[] }) {
  const [pendingClusters, setPendingClusters] = useState(clusters);
  const [people, setPeople] = useState(initialPeople);
  const [message, setMessage] = useState("");
  const [pendingDeletion, setPendingDeletion] = useState<PendingDeletion | null>(null);
  /** Voices left out of their group's naming, by cluster ID. */
  const [leftOut, setLeftOut] = useState<Set<string>>(new Set());

  /** Asks before deleting every voice of a group (often just one), then removes its card. */
  function askToDeleteGroup(group: VoiceGroup, speakerLabel: string) {
    const count = group.voices.length;
    setPendingDeletion({
      title: `Delete ${speakerLabel}?`,
      message:
        count === 1
          ? "Its voice sample goes for good, here and on your paired devices. The transcript stays."
          : `Its ${count} voice samples go for good, here and on your paired devices. The transcripts stay.`,
      run: async () => {
        for (const voice of group.voices) {
          await deleteUnknownVoice(voice.cluster_id);
          setPendingClusters((current) => current.filter((candidate) => candidate.cluster_id !== voice.cluster_id));
        }
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

  /** Includes a voice in, or leaves it out of, its group's naming. */
  function setIncluded(clusterId: string, included: boolean) {
    setLeftOut((current) => {
      const next = new Set(current);
      if (included) next.delete(clusterId);
      else next.add(clusterId);
      return next;
    });
  }

  /** Removes named voices and adds or updates their person in the saved list. */
  function recordNaming(namedIds: string[], identityId: string, identityName: string, variantsStored: number) {
    setPendingClusters((current) => current.filter((candidate) => !namedIds.includes(candidate.cluster_id)));
    setPeople((current) => [
      ...current.filter((person) => person.identity_id !== identityId),
      {
        identity_id: identityId,
        display_name: identityName,
        voice_variant_count: (current.find((person) => person.identity_id === identityId)?.voice_variant_count ?? 0) + variantsStored
      }
    ].sort((first, second) => first.display_name.localeCompare(second.display_name)));
  }

  /** Names a lone voice, or the included voices of a group, after the typed name. */
  async function submitAssignment(event: FormEvent<HTMLFormElement>, group: VoiceGroup) {
    event.preventDefault();
    const identityName = String(new FormData(event.currentTarget).get("identityName") ?? "").trim();
    if (!identityName) {
      setMessage("Enter a name before assigning.");
      return;
    }
    const noVoiceNote = " There was not enough clear speech to recognize this voice in future recordings.";
    try {
      if (group.voices.length === 1) {
        const assignment = await assignCluster(group.voices[0].cluster_id, identityName);
        recordNaming([assignment.cluster_id], assignment.identity_id, assignment.identity_name, assignment.variants_stored);
        setMessage(`Assigned to ${assignment.identity_name}.${assignment.variants_stored > 0 ? "" : noVoiceNote}`);
        return;
      }
      const included = group.voices.filter((voice) => !leftOut.has(voice.cluster_id)).map((voice) => voice.cluster_id);
      if (included.length === 0) {
        setMessage("Choose at least one voice to name.");
        return;
      }
      const assignment = await assignVoiceGroup(included, identityName);
      recordNaming(assignment.assigned_cluster_ids, assignment.identity_id, assignment.identity_name, assignment.variants_stored);
      const named = assignment.assigned_cluster_ids.length;
      const assigned = named === 1 ? `Assigned to ${assignment.identity_name}.` : `Assigned ${named} voices to ${assignment.identity_name}.`;
      setMessage(`${assigned}${assignment.variants_stored > 0 ? "" : noVoiceNote}`);
    } catch {
      setMessage("Unable to assign the speaker cluster.");
    }
  }

  const groups = groupVoices(pendingClusters);

  return (
    <section className="space-y-4">
      <h1 className="text-3xl font-bold">Voice gallery</h1>
      <datalist id="saved-people">
        {people.map((person) => <option key={person.identity_id} value={person.display_name} />)}
      </datalist>
      <h2 className="text-lg font-semibold">Unknown speakers</h2>
      {groups.length === 0 ? <p>No unknown speakers to review.</p> : null}
      {groups.map((group, index) => {
        const speakerLabel = `Unknown speaker ${index + 1}`;
        const lone = group.voices.length === 1 ? group.voices[0] : null;
        const longestPlayable = group.voices.filter((voice) => voice.has_review_clip).sort((first, second) => second.total_duration_ms - first.total_duration_ms)[0];
        return (
          <article className="card space-y-2" key={group.groupId}>
            <div className="flex items-center justify-between gap-2">
              <h3 className="font-semibold">{speakerLabel}</h3>
              <button
                type="button"
                aria-label={`Delete ${speakerLabel}`}
                onClick={() => askToDeleteGroup(group, speakerLabel)}
                className="min-h-11 rounded-lg px-3 font-medium text-danger hover:bg-surface-muted"
              >
                Delete
              </button>
            </div>
            <p>{lone ? formatSummary(lone) : formatGroupSummary(group)}</p>
            <p>{lone ? lone.meeting_title ?? "Untitled meeting" : [...new Set(group.voices.map((voice) => voice.meeting_title ?? "Untitled meeting"))].join(", ")}</p>
            {longestPlayable ? <audio controls preload="none" src={reviewClipUrl(longestPlayable.cluster_id)} aria-label={`Listen to ${speakerLabel.toLowerCase()}`} /> : null}
            {lone ? null : (
              <details className="rounded-lg border border-line-strong px-3 py-2">
                <summary className="min-h-11 cursor-pointer py-2 font-medium">{group.voices.length} parts</summary>
                <p className="text-sm">These sound like one person. Untick any that is someone else.</p>
                <ul className="space-y-2 pt-2">
                  {group.voices.map((voice) => (
                    <li key={voice.cluster_id} className="flex flex-wrap items-center gap-2">
                      <label className="flex min-h-11 items-center gap-2">
                        <input type="checkbox" className="h-5 w-5" checked={!leftOut.has(voice.cluster_id)} onChange={(event) => setIncluded(voice.cluster_id, event.target.checked)} aria-label={`Include ${partLabel(voice)}`} />
                        <span>{partLabel(voice)}</span>
                      </label>
                      {voice.has_review_clip ? <audio controls preload="none" src={reviewClipUrl(voice.cluster_id)} aria-label={`Listen to ${partLabel(voice)}`} className="max-w-full" /> : null}
                    </li>
                  ))}
                </ul>
              </details>
            )}
            <form className="space-y-2" onSubmit={(event) => submitAssignment(event, group)}>
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
