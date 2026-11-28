// Renders the web Settings page's glossary: names, project names and terms, one per line, that
// transcription is prompted with so they are spelled as written. It applies from the next recording.

"use client";

import { useId, useState } from "react";
import { GlossaryError, setGlossary, type GlossaryFailureKind } from "@/lib/api";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";
import { buttonStyles } from "@/components/ui";

/** Plain messages per failure; server text is never shown because it can contain internal details. */
const failureMessages = (t: Translate) : Record<GlossaryFailureKind, string> => ({
  invalid_glossary: t("eachTermCanHaveAt"),
  unavailable: t("thisServerHasNoGlossary"),
  server: t("somethingWentWrongOnThe"),
  network: t("cantReachTheNoteTaking")
});

export type GlossarySettingsProps = {
  /** The saved terms, or `null` when they could not be read. */
  initialTerms: string[] | null;
};

/**
 * Lets the owner edit the glossary.
 *
 * Inputs: see `GlossarySettingsProps`. Outputs: a text area (one term per line), a Save button and
 * a status line announced to screen readers. Side effects: `PUT /api/settings/glossary` on Save;
 * text the server refuses stays in the field to correct.
 */
export function GlossarySettings({ initialTerms }: GlossarySettingsProps) {
  const t = useT();
  const [text, setText] = useState((initialTerms ?? []).join("\n"));
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initialTerms === null ? t("cantReadTheGlossaryCheck") : null
  );
  const fieldId = useId();
  const hintId = useId();

  /** Saves the lines as terms and shows them as the server kept them. */
  async function save() {
    const terms = text
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.length > 0);
    setIsSaving(true);
    setMessage(null);
    try {
      const saved = await setGlossary(terms);
      setText(saved.join("\n"));
      setMessage(t("savedTheNextRecordingUses"));
    } catch (error) {
      setMessage(failureMessages(t)[error instanceof GlossaryError ? error.kind : "server"]);
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="card max-w-2xl space-y-3">
      <div className="space-y-1">
        <label htmlFor={fieldId} className="block font-semibold">{t("glossary")}</label>
        <p id={hintId} className="text-sm text-muted">{t("namesProjectNamesAndTerms")}</p>
        <textarea
          id={fieldId}
          aria-describedby={hintId}
          className="block min-h-28 w-full rounded-lg border border-line-strong bg-surface px-3 py-2 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30 disabled:text-faint"
          placeholder={"Marco Rossi\nKubernetes\nOKR"}
          value={text}
          disabled={initialTerms === null || isSaving}
          onChange={(event) => setText(event.target.value)}
        />
      </div>
      <button
        type="button"
        className={buttonStyles.filled}
        disabled={initialTerms === null || isSaving}
        onClick={save}
      >
        {isSaving ? t("saving") : t("saveGlossary")}
      </button>
      <p role="status" aria-live="polite">
        {message}
      </p>
    </div>
  );
}
