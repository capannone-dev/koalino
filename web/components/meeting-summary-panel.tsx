// Renders a meeting's AI summary (#95) on its page: the stored summary, a button to write or
// rewrite it with the default connection or another one, and the meeting's own prompt, which
// replaces the Settings prompt for this meeting only.

"use client";

import { useId, useState } from "react";
import {
  llmErrorMessage,
  setMeetingPrompt,
  summarizeMeeting,
  type LlmConnection,
  type StoredSummary
} from "@/lib/llm-api";

const buttonClass =
  "inline-flex min-h-11 items-center justify-center rounded-lg border border-line-strong px-4 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus disabled:text-faint";
const fieldClass =
  "min-h-11 w-full rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30";

export type MeetingSummaryPanelProps = {
  meetingId: string;
  /** The stored summary, or `null` when there is none yet. */
  summary: StoredSummary | null;
  /** The meeting's own prompt, or `null` when it uses the Settings prompt. */
  meetingPrompt: string | null;
  /** The saved connections, or `null` when AI settings are unavailable. */
  connections: LlmConnection[] | null;
  /** The Settings prompt, shown as a starting point for the meeting's own. */
  defaultTemplate: string;
};

/**
 * Shows and writes a meeting's summary.
 *
 * Inputs: see `MeetingSummaryPanelProps`. Outputs: the summary as text, the summarize controls,
 * the prompt editor, and a status line for screen readers. Side effects: "Summarize" sends the
 * meeting's transcript, notes and photos to the chosen connection's server and stores the result
 * in the meeting, which syncs to paired devices; saving a prompt writes it to the meeting.
 */
export function MeetingSummaryPanel({
  meetingId,
  summary,
  meetingPrompt,
  connections,
  defaultTemplate
}: MeetingSummaryPanelProps) {
  const [current, setCurrent] = useState(summary);
  const [connectionId, setConnectionId] = useState(
    connections?.find((connection) => connection.is_default)?.id ?? connections?.[0]?.id ?? ""
  );
  const [prompt, setPrompt] = useState(meetingPrompt);
  const [editingPrompt, setEditingPrompt] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const connectionFieldId = useId();
  const promptFieldId = useId();

  /** Writes the summary with the chosen connection. */
  async function summarize() {
    setBusy(true);
    setMessage("Writing the summary… This can take a few minutes with a local model.");
    try {
      const result = await summarizeMeeting(meetingId, connectionId || undefined);
      setCurrent(result.summary);
      const notes = [
        result.parts > 1 ? `The transcript was read in ${result.parts} parts.` : null,
        result.images_skipped > 0 ? `${result.images_skipped} picture(s) could not be sent.` : null
      ].filter(Boolean);
      setMessage(["Summary written.", ...notes].join(" "));
    } catch (error) {
      setMessage(llmErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  /** Saves or clears the meeting's own prompt. */
  async function savePrompt(text: string | null) {
    setBusy(true);
    setMessage(null);
    try {
      await setMeetingPrompt(meetingId, text);
      setPrompt(text);
      setEditingPrompt(null);
      setMessage(text === null ? "This meeting uses the Settings prompt again." : "Prompt saved for this meeting.");
    } catch (error) {
      setMessage(llmErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const unavailable = connections === null;
  const noConnection = connections !== null && connections.length === 0;
  return (
    <section id="summary" aria-labelledby={`${meetingId}-summary-heading`} className="card space-y-3">
      <h2 id={`${meetingId}-summary-heading`}>Summary</h2>
      {current ? (
        <div className="space-y-1">
          <div className="whitespace-pre-wrap break-words">{current.text}</div>
          <p className="text-sm text-muted">
            Written by {current.model} ({current.connection_label})
            {current.edited ? ", edited" : ""}
            {current.truncated ? ". The model stopped early, so the end may be missing" : ""}.
          </p>
        </div>
      ) : (
        <p className="text-muted">No summary yet.</p>
      )}
      {unavailable ? <p className="text-muted">AI connections are unavailable on this server.</p> : null}
      {noConnection ? <p className="text-muted">Add an AI connection in Settings to write summaries.</p> : null}
      {connections && connections.length > 0 ? (
        <div className="flex flex-wrap items-end gap-2">
          <div>
            <label htmlFor={connectionFieldId} className="block text-sm font-semibold">
              Summarize with
            </label>
            <select
              id={connectionFieldId}
              className={fieldClass}
              value={connectionId}
              disabled={busy}
              onChange={(event) => setConnectionId(event.target.value)}
            >
              {connections.map((connection) => (
                <option key={connection.id} value={connection.id}>
                  {connection.label} ({connection.model})
                </option>
              ))}
            </select>
          </div>
          <button type="button" className={buttonClass} disabled={busy} onClick={() => void summarize()}>
            {current ? "Summarize again" : "Summarize"}
          </button>
        </div>
      ) : null}
      {editingPrompt === null ? (
        <div className="flex flex-wrap items-center gap-2">
          <p className="text-sm text-muted">
            {prompt === null ? "Uses the prompt from Settings." : "Uses its own prompt."}
          </p>
          <button type="button" className={buttonClass} disabled={busy} onClick={() => setEditingPrompt(prompt ?? defaultTemplate)}>
            Edit prompt for this meeting
          </button>
          {prompt === null ? null : (
            <button type="button" className={buttonClass} disabled={busy} onClick={() => void savePrompt(null)}>
              Use the Settings prompt
            </button>
          )}
        </div>
      ) : (
        <div className="space-y-2">
          <label htmlFor={promptFieldId} className="block font-semibold">
            Prompt for this meeting
          </label>
          <textarea
            id={promptFieldId}
            className={`${fieldClass} min-h-40 py-2`}
            value={editingPrompt}
            onChange={(event) => setEditingPrompt(event.target.value)}
          />
          <div className="flex flex-wrap gap-2">
            <button type="button" className={buttonClass} disabled={busy} onClick={() => void savePrompt(editingPrompt)}>
              Save prompt
            </button>
            <button type="button" className={buttonClass} onClick={() => setEditingPrompt(null)}>
              Cancel
            </button>
          </div>
        </div>
      )}
      <p role="status" aria-live="polite">
        {message}
      </p>
    </section>
  );
}
