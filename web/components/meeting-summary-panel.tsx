// Renders a meeting's AI summary (#95) on its page: the stored summary, a button to write or
// rewrite it with the default connection or another one, and the meeting's own prompt, which
// replaces the Settings prompt for this meeting only. The owner can also edit the summary's text,
// or write one by hand (#96); an edited summary is never replaced automatically.

"use client";

import { useId, useState } from "react";
import { editErrorMessage, editSummaryText } from "@/lib/edits-api";
import {
  llmErrorMessage,
  setMeetingPrompt,
  summarizeMeeting,
  type LlmConnection,
  type StoredSummary
} from "@/lib/llm-api";
import { useT } from "@/lib/i18n/client";

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
  const t = useT();
  const [current, setCurrent] = useState(summary);
  const [connectionId, setConnectionId] = useState(
    connections?.find((connection) => connection.is_default)?.id ?? connections?.[0]?.id ?? ""
  );
  const [prompt, setPrompt] = useState(meetingPrompt);
  const [editingPrompt, setEditingPrompt] = useState<string | null>(null);
  const [editingText, setEditingText] = useState<string | null>(null);
  const textFieldId = useId();
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const connectionFieldId = useId();
  const promptFieldId = useId();

  /** Writes the summary with the chosen connection. */
  async function summarize() {
    setBusy(true);
    setMessage(t("writingTheSummaryThisCan"));
    try {
      const result = await summarizeMeeting(meetingId, connectionId || undefined);
      setCurrent(result.summary);
      const notes = [
        result.parts > 1 ? t("theTranscriptWasReadIn", { parts: result.parts }) : null,
        result.images_skipped > 0 ? t("picturesNotSent", { count: result.images_skipped }) : null
      ].filter(Boolean);
      setMessage([t("summaryWritten"), ...notes].join(" "));
    } catch (error) {
      setMessage(llmErrorMessage(error, t));
    } finally {
      setBusy(false);
    }
  }

  /** Saves the summary text the owner edited or wrote. */
  async function saveText(text: string) {
    setBusy(true);
    setMessage(null);
    try {
      setCurrent(await editSummaryText(meetingId, text));
      setEditingText(null);
      setMessage(t("summarySaved"));
    } catch (error) {
      setMessage(editErrorMessage(error, t));
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
      setMessage(text === null ? t("thisMeetingUsesTheSettings") : t("promptSavedForThisMeeting"));
    } catch (error) {
      setMessage(llmErrorMessage(error, t));
    } finally {
      setBusy(false);
    }
  }

  const unavailable = connections === null;
  const noConnection = connections !== null && connections.length === 0;
  return (
    <section id="summary" aria-labelledby={`${meetingId}-summary-heading`} className="card space-y-3">
      <h2 id={`${meetingId}-summary-heading`}>{t("summary")}</h2>
      {editingText !== null ? (
        <div className="space-y-2">
          <label htmlFor={textFieldId} className="block font-semibold">{t("summaryMarkdown")}</label>
          <textarea
            id={textFieldId}
            className={`${fieldClass} min-h-60 py-2 font-mono`}
            value={editingText}
            onChange={(event) => setEditingText(event.target.value)}
          />
          <div className="flex flex-wrap gap-2">
            <button type="button" className={buttonClass} disabled={busy} onClick={() => void saveText(editingText)}>{t("saveSummary")}</button>
            <button type="button" className={buttonClass} disabled={busy} onClick={() => setEditingText(null)}>{t("cancel")}</button>
          </div>
        </div>
      ) : current ? (
        <div className="space-y-1">
          <div className="whitespace-pre-wrap break-words">{current.text}</div>
          <p className="text-sm text-muted">
            {current.connection_label === "" && current.model === "" ? (
              t("writtenByYou")
            ) : (
              <>{t("writtenBy", { model: current.model, connectionLabel: current.connection_label, edited: current.edited ? t("edited") : "", stopped: current.truncated ? t("modelStoppedEarly") : "" })}</>
            )}
          </p>
          <button type="button" className={buttonClass} disabled={busy} onClick={() => setEditingText(current.text)}>{t("editSummary")}</button>
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <p className="text-muted">{t("noSummaryYet")}</p>
          <button type="button" className={buttonClass} disabled={busy} onClick={() => setEditingText("")}>{t("writeASummary")}</button>
        </div>
      )}
      {unavailable ? <p className="text-muted">{t("aiConnectionsAreUnavailableOn")}</p> : null}
      {noConnection ? <p className="text-muted">{t("addAnAiConnectionIn")}</p> : null}
      {connections && connections.length > 0 ? (
        <div className="flex flex-wrap items-end gap-2">
          <div>
            <label htmlFor={connectionFieldId} className="block text-sm font-semibold">{t("summarizeWith")}</label>
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
            {current ? t("summarizeAgain") : t("summarize")}
          </button>
        </div>
      ) : null}
      {editingPrompt === null ? (
        <div className="flex flex-wrap items-center gap-2">
          <p className="text-sm text-muted">
            {prompt === null ? t("usesThePromptFromSettings") : t("usesItsOwnPrompt")}
          </p>
          <button type="button" className={buttonClass} disabled={busy} onClick={() => setEditingPrompt(prompt ?? defaultTemplate)}>{t("editPromptForThisMeeting")}</button>
          {prompt === null ? null : (
            <button type="button" className={buttonClass} disabled={busy} onClick={() => void savePrompt(null)}>{t("useTheSettingsPrompt")}</button>
          )}
        </div>
      ) : (
        <div className="space-y-2">
          <label htmlFor={promptFieldId} className="block font-semibold">{t("promptForThisMeeting")}</label>
          <textarea
            id={promptFieldId}
            className={`${fieldClass} min-h-40 py-2`}
            value={editingPrompt}
            onChange={(event) => setEditingPrompt(event.target.value)}
          />
          <div className="flex flex-wrap gap-2">
            <button type="button" className={buttonClass} disabled={busy} onClick={() => void savePrompt(editingPrompt)}>{t("savePrompt")}</button>
            <button type="button" className={buttonClass} onClick={() => setEditingPrompt(null)}>{t("cancel")}</button>
          </div>
        </div>
      )}
      <p role="status" aria-live="polite">
        {message}
      </p>
    </section>
  );
}
