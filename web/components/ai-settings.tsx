// Renders the web Settings page's AI connections (#95): the language models summaries are written
// with (a server on the local network, or Claude Code and Codex signed in on this computer), the
// default one, the summary prompt with its placeholders, whether meetings are summarized
// automatically (#101), and the connection that answers paired devices' summary requests (#107).
// The app keeps no API keys (#117).

"use client";

import { FormEvent, useId, useState } from "react";
import {
  connectionPresets,
  deleteLlmConnection,
  getLlmSettings,
  llmErrorMessage,
  saveLlmConnection,
  testLlmConnection,
  updateLlmSettings,
  type LlmConnection,
  type LlmConnectionInput,
  type LlmKind,
  isLocalCli,
  type LlmSettings
} from "@/lib/llm-api";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";

const fieldClass =
  "min-h-11 w-full rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30";
const buttonClass =
  "inline-flex min-h-11 items-center justify-center rounded-lg border border-line-strong px-4 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus disabled:text-faint";

/** The connection form's fields; `id` is set when editing, `has_key` when a key is saved. */
type Draft = LlmConnectionInput & { id?: string; has_key?: boolean; description?: string };

/** An empty form, starting from the first preset. */
function emptyDraft(t: Translate): Draft {
  return { label: "", ...connectionPresets(t)[0].input };
}

export type AiSettingsProps = {
  /** The saved settings, or `null` when they could not be read. */
  initial: LlmSettings | null;
};

/**
 * Lets the owner manage AI connections and the summary prompt.
 *
 * Inputs: see `AiSettingsProps`. Outputs: the connection list, a form to add or edit one, the
 * prompt editor and the automatic-summary switch, with a status line for screen readers.
 * Side effects: calls the `/api/settings/llm` routes; "Test" sends one line, no meeting content.
 */
export function AiSettings({ initial }: AiSettingsProps) {
  const t = useT();
  const [settings, setSettings] = useState<LlmSettings | null>(initial);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [template, setTemplate] = useState(initial?.template ?? "");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initial === null ? t("cantReadTheAiSettings") : null
  );
  const templateId = useId();
  const autoId = useId();
  const pairedId = useId();

  if (settings === null) {
    return (
      <div className="card max-w-2xl space-y-2">
        <h2 className="font-semibold">{t("aiConnections")}</h2>
        <p role="status">{message}</p>
      </div>
    );
  }

  /** Runs an action, showing its outcome or a plain error. */
  async function run(action: () => Promise<string | void>) {
    setBusy(true);
    setMessage(null);
    try {
      const done = await action();
      if (done) setMessage(done);
    } catch (error) {
      setMessage(llmErrorMessage(error, t));
    } finally {
      setBusy(false);
    }
  }

  /** Replaces one connection in the list, or adds it. */
  function withConnection(current: LlmSettings, saved: LlmConnection): LlmSettings {
    const known = current.connections.some((connection) => connection.id === saved.id);
    const connections = known
      ? current.connections.map((connection) => (connection.id === saved.id ? saved : connection))
      : [...current.connections, saved];
    return { ...current, connections };
  }

  /** Saves the form. */
  function submit(event: FormEvent) {
    event.preventDefault();
    if (!draft) return;
    const body: LlmConnectionInput = {
      label: draft.label,
      kind: draft.kind,
      base_url: draft.base_url,
      model: draft.model,
      context_tokens: Number(draft.context_tokens)
    };
    // An online service's key (#138), only for Chat Completions; blank keeps the saved one.
    if (draft.kind === "open_ai_compatible") {
      const key = draft.api_key?.trim();
      if (key) body.api_key = key;
      if (draft.clear_key) body.clear_key = true;
    }
    const id = draft.id;
    // Claude Code and Codex have no address (#105).
    if (isLocalCli(body.kind)) body.base_url = "";
    void run(async () => {
      const saved = await saveLlmConnection(body, id);
      // The first connection becomes the default, so the whole list is read again.
      const refreshed = await getLlmSettings().catch(() => null);
      setSettings((current) => refreshed ?? (current ? withConnection(current, saved) : current));
      setDraft(null);
      return t("saved", { label: saved.label });
    });
  }

  const current = settings;
  return (
    <div className="card max-w-2xl space-y-4">
      <div className="space-y-1">
        <h2 className="font-semibold">{t("aiConnections")}</h2>
        <p className="text-sm text-muted">{t("summariesAreWrittenByA")}</p>
      </div>

      {current.connections.length === 0 ? <p className="text-muted">{t("noConnectionYet")}</p> : null}
      <ul className="space-y-2">
        {current.connections.map((connection) => (
          <li key={connection.id} className="rounded-lg border border-line p-3">
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-semibold">{connection.label}</span>
              {connection.is_default ? <span className="text-sm text-success">{t("default")}</span> : null}
              <span className="text-sm text-muted">
                {isLocalCli(connection.kind)
                  ? `${connection.kind === "codex" ? "Codex" : "Claude Code"} · ${connection.model || t("defaultModel")}`
                  : connection.model}
              </span>
            </div>
            {isLocalCli(connection.kind) ? (
              <p className="text-sm text-muted">{t("runsWithThatComputersSigned", { status: connection.tool_installed ? t("installedOnTheServersComputer") : t("notFoundOnTheServers") })}</p>
            ) : (
              <>
                <p className="text-sm break-all text-muted">{connection.base_url}</p>
                {connection.plain_http ? (
                  <p className="text-sm text-warning">{t("unencryptedHttpUseItOnly")}</p>
                ) : null}
              </>
            )}
            <div className="mt-2 flex flex-wrap gap-2">
              {connection.is_default ? null : (
                <button
                  type="button"
                  className={buttonClass}
                  disabled={busy}
                  onClick={() =>
                    void run(async () => {
                      setSettings(await updateLlmSettings({ default_connection: connection.id }));
                      return t("isNowTheDefault", { label: connection.label });
                    })
                  }
                >{t("makeDefault")}</button>
              )}
              <button
                type="button"
                className={buttonClass}
                disabled={busy}
                onClick={() =>
                  void run(async () => `${connection.label} answered: ${await testLlmConnection(connection.id)}`)
                }
              >{t("test")}</button>
              <button
                type="button"
                className={buttonClass}
                disabled={busy}
                onClick={() => setDraft({ id: connection.id, label: connection.label, kind: connection.kind, base_url: connection.base_url, model: connection.model, context_tokens: connection.context_tokens, has_key: connection.has_key })}
              >{t("edit")}</button>
              <button
                type="button"
                className={`${buttonClass} text-danger`}
                disabled={busy}
                onClick={() =>
                  void run(async () => {
                    await deleteLlmConnection(connection.id);
                    setSettings((settingsNow) =>
                      settingsNow
                        ? { ...settingsNow, connections: settingsNow.connections.filter((item) => item.id !== connection.id) }
                        : settingsNow
                    );
                    return t("removed", { label: connection.label });
                  })
                }
              >{t("remove")}</button>
            </div>
          </li>
        ))}
      </ul>

      {draft ? (
        <ConnectionForm draft={draft} busy={busy} onChange={setDraft} onSubmit={submit} onCancel={() => setDraft(null)} />
      ) : (
        <button type="button" className={buttonClass} onClick={() => setDraft(emptyDraft(t))}>{t("addConnection")}</button>
      )}

      {/* The prompt and when summaries are written: for those who want to change them (#209). */}
      <details className="rounded-lg border border-line px-3">
        <summary className="min-h-11 cursor-pointer content-center font-semibold focus-visible:outline-2 focus-visible:outline-focus">
          {t("summaryOptions")}
        </summary>
        <div className="space-y-4 pb-3 pt-2">
      <div className="space-y-1">
        <label htmlFor={templateId} className="block font-semibold">{t("summaryPrompt")}</label>
        <p className="text-sm text-muted">{t("placeholdersTheTranscriptIsAlways", { placeholders: current.placeholders.map((name) => `{{${name}}}`).join(", ") })}</p>
        <textarea
          id={templateId}
          className={`${fieldClass} min-h-40 py-2`}
          value={template}
          onChange={(event) => setTemplate(event.target.value)}
        />
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            className={buttonClass}
            disabled={busy}
            onClick={() =>
              void run(async () => {
                const saved = await updateLlmSettings({ template });
                setSettings(saved);
                return t("promptSaved");
              })
            }
          >{t("savePrompt")}</button>
          <button
            type="button"
            className={buttonClass}
            disabled={busy || !current.custom_template}
            onClick={() =>
              void run(async () => {
                const saved = await updateLlmSettings({ template: null });
                setSettings(saved);
                setTemplate(saved.template);
                return t("backToTheBuiltIn");
              })
            }
          >{t("resetToBuiltIn")}</button>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <input
          id={autoId}
          type="checkbox"
          className="h-5 w-5"
          checked={current.auto_summarize}
          disabled={busy}
          onChange={(event) => {
            const enabled = event.target.checked;
            void run(async () => {
              setSettings(await updateLlmSettings({ auto_summarize: enabled }));
              return enabled ? t("meetingsWillBeSummarizedAutomatically") : t("automaticSummariesAreOff");
            });
          }}
        />
        <label htmlFor={autoId}>{t("summarizeMeetingsAutomaticallyWithThe")}</label>
      </div>

      {current.connections.length > 0 && (
        <div className="flex flex-col gap-1">
          <label htmlFor={pairedId}>{t("summariesForMyOtherDevices")}</label>
          <select
            id={pairedId}
            className="w-full max-w-sm rounded border px-2 py-2"
            value={current.paired_summary_connection ?? ""}
            disabled={busy}
            onChange={(event) => {
              const id = event.target.value === "" ? null : event.target.value;
              const label = current.connections.find((connection) => connection.id === id)?.label;
              void run(async () => {
                setSettings(await updateLlmSettings({ paired_summary_connection: id }));
                return id === null
                  ? t("thisComputerNoLongerWrites")
                  : t("nowWritesTheSummariesAnd", { label: label ?? "The connection" });
              });
            }}
          >
            <option value="">{t("off")}</option>
            {current.connections.map((connection) => (
              <option key={connection.id} value={connection.id}>
                {connection.label}
              </option>
            ))}
          </select>
          <p className="text-sm opacity-80">{t("aPairedPhoneCanAsk")}</p>
        </div>
      )}
        </div>
      </details>

      <p role="status" aria-live="polite">
        {message}
      </p>
    </div>
  );
}

/** The add/edit form for one connection. */
function ConnectionForm({
  draft,
  busy,
  onChange,
  onSubmit,
  onCancel
}: {
  draft: Draft;
  busy: boolean;
  onChange: (draft: Draft) => void;
  onSubmit: (event: FormEvent) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const ids = { preset: useId(), label: useId(), kind: useId(), url: useId(), model: useId(), context: useId(), key: useId(), keyHelp: useId() };
  return (
    <form className="space-y-3 rounded-lg border border-line p-3" onSubmit={onSubmit}>
      <h3 className="font-semibold">{draft.id ? t("editConnection") : t("newConnection")}</h3>
      {draft.id ? null : (
        <div>
          <label htmlFor={ids.preset} className="block text-sm font-semibold">{t("startFrom")}</label>
          <select
            id={ids.preset}
            className={fieldClass}
            defaultValue={connectionPresets(t)[0].name}
            onChange={(event) => {
              const preset = connectionPresets(t).find((candidate) => candidate.name === event.target.value);
              if (preset) onChange({ ...draft, ...preset.input, description: preset.description });
            }}
          >
            {connectionPresets(t).map((preset) => (
              <option key={preset.name}>{preset.name}</option>
            ))}
          </select>
          {draft.description ? <p className="mt-1 text-sm text-muted">{draft.description}</p> : null}
        </div>
      )}
      <div>
        <label htmlFor={ids.label} className="block text-sm font-semibold">{t("name")}</label>
        <input id={ids.label} className={fieldClass} required value={draft.label} onChange={(event) => onChange({ ...draft, label: event.target.value })} />
      </div>
      <div>
        <label htmlFor={ids.kind} className="block text-sm font-semibold">{t("protocol")}</label>
        <select id={ids.kind} className={fieldClass} value={draft.kind} onChange={(event) => onChange({ ...draft, kind: event.target.value as LlmKind })}>
          <option value="open_ai_compatible">{t("openaiCompatibleServerOllamaLm")}</option>
          <option value="claude_code">{t("claudeCodeOnTheServers")}</option>
          <option value="codex">{t("codexOnTheServersComputer")}</option>
        </select>
      </div>
      {isLocalCli(draft.kind) ? (
        <p className="text-sm text-muted">{t("runsTheToolInstalledAnd")}</p>
      ) : (
        <div>
          <label htmlFor={ids.url} className="block text-sm font-semibold">{t("address")}</label>
          <input id={ids.url} className={fieldClass} required value={draft.base_url} onChange={(event) => onChange({ ...draft, base_url: event.target.value })} />
        </div>
      )}
      <div>
        <label htmlFor={ids.model} className="block text-sm font-semibold">{t("model")}</label>
        <input
          id={ids.model}
          className={fieldClass}
          required={!isLocalCli(draft.kind)}
          placeholder={isLocalCli(draft.kind) ? t("emptyTheToolsOwnDefault") : undefined}
          value={draft.model}
          onChange={(event) => onChange({ ...draft, model: event.target.value })}
        />
      </div>
      <div>
        <label htmlFor={ids.context} className="block text-sm font-semibold">{t("contextSizeTokens")}</label>
        <input
          id={ids.context}
          type="number"
          min={2048}
          max={2000000}
          className={fieldClass}
          required
          value={draft.context_tokens}
          onChange={(event) => onChange({ ...draft, context_tokens: Number(event.target.value) })}
        />
      </div>
      {draft.kind === "open_ai_compatible" ? (
        <div>
          <label htmlFor={ids.key} className="block text-sm font-semibold">{t("apiKey")}</label>
          <input
            id={ids.key}
            type="password"
            autoComplete="off"
            className={fieldClass}
            aria-describedby={ids.keyHelp}
            disabled={draft.clear_key}
            value={draft.api_key ?? ""}
            onChange={(event) => onChange({ ...draft, api_key: event.target.value })}
          />
          <p id={ids.keyHelp} className="mt-1 text-sm text-muted">
            {draft.has_key ? t("aKeyIsSavedLeaveEmpty") : t("onlyForOnlineServicesKeptIn")}
          </p>
          {draft.has_key ? (
            <label className="mt-1 flex items-center gap-2 text-sm">
              <input type="checkbox" checked={draft.clear_key ?? false} onChange={(event) => onChange({ ...draft, clear_key: event.target.checked })} />
              {t("removeTheSavedKey")}
            </label>
          ) : null}
        </div>
      ) : null}
      <div className="flex flex-wrap gap-2">
        <button type="submit" className={buttonClass} disabled={busy}>{t("saveConnection")}</button>
        <button type="button" className={buttonClass} onClick={onCancel}>{t("cancel")}</button>
      </div>
    </form>
  );
}
