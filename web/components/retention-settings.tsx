// Renders the web Settings page's speech retention window (#93): how many days a recording's
// speech may wait to be combined with another device's recording of the same meeting before it
// is deleted.

"use client";

import { useId, useState } from "react";
import { RetentionError, setRetention, type RetentionFailureKind, type SpeechRetention } from "@/lib/api";

/** Plain messages per failure; server text is never shown because it can contain internal details. */
const failureMessages: Record<RetentionFailureKind, string> = {
  invalid_window: "Choose a window between 1 and 60 days.",
  unavailable: "This server keeps no speech for combining.",
  server: "Something went wrong on the server. Try again.",
  network: "Can't reach the note-taking server. Check that it is running."
};

/** Common windows, in days, offered between the bounds. */
const commonWindows = [3, 7, 14, 21, 30, 45];

/**
 * Lists the windows the menu offers.
 *
 * Inputs: the window and its bounds. Outputs: the bounds, the common windows between them, and
 * the current window (it may have been set from the CLI), in increasing order. Side effects: none.
 */
export function retentionChoices(current: SpeechRetention): number[] {
  const choices = new Set([
    current.min_days,
    ...commonWindows.filter((days) => days > current.min_days && days < current.max_days),
    current.max_days,
    current.days
  ]);
  return Array.from(choices).sort((left, right) => left - right);
}

export type RetentionSettingsProps = {
  /** The saved window, or `null` when it could not be read. */
  initial: SpeechRetention | null;
};

/**
 * Lets the owner choose how long speech waits to be combined.
 *
 * Inputs: see `RetentionSettingsProps`. Outputs: a labelled menu of windows and a status line
 * announced to screen readers. Side effects: `PUT /api/settings/retention` when a window is
 * chosen; a refused choice goes back to the saved window.
 */
export function RetentionSettings({ initial }: RetentionSettingsProps) {
  const [retention, setRetentionState] = useState<SpeechRetention>(initial ?? { days: 21, min_days: 1, max_days: 60 });
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initial === null ? "Can't read how long speech is kept. Check that the note-taking server is running." : null
  );
  const fieldId = useId();
  const hintId = useId();

  /** Saves the chosen window and shows it as the server kept it. */
  async function choose(days: number) {
    setIsSaving(true);
    setMessage(null);
    try {
      const saved = await setRetention(days);
      setRetentionState(saved);
      setMessage(`Saved. Speech now waits up to ${saved.days} ${saved.days === 1 ? "day" : "days"} to be combined.`);
    } catch (error) {
      setMessage(failureMessages[error instanceof RetentionError ? error.kind : "server"]);
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="card max-w-2xl space-y-3">
      <div className="space-y-1">
        <label htmlFor={fieldId} className="block font-semibold">
          Keep speech for combining
        </label>
        <p id={hintId} className="text-sm text-muted">
          When a meeting was also recorded on another device, its speech waits this long to be combined, then is
          deleted.
        </p>
        <select
          id={fieldId}
          aria-describedby={hintId}
          className="min-h-11 rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30 disabled:text-faint"
          value={String(retention.days)}
          disabled={initial === null || isSaving}
          onChange={(event) => void choose(Number(event.target.value))}
        >
          {retentionChoices(retention).map((days) => (
            <option key={days} value={String(days)}>
              {days === 1 ? "1 day" : `${days} days`}
            </option>
          ))}
        </select>
      </div>
      <p role="status" aria-live="polite">
        {message}
      </p>
    </div>
  );
}
