// Renders the web Settings page's AI connections (#95): the language models summaries are written
// with (OpenAI, Anthropic, Mistral or a server on the local network), the default one, the
// summary prompt with its placeholders, and whether meetings are summarized automatically (#101).
// API keys can be typed in but are never shown again.

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

const fieldClass =
  "min-h-11 w-full rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30";
const buttonClass =
  "inline-flex min-h-11 items-center justify-center rounded-lg border border-line-strong px-4 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus disabled:text-faint";

/** The connection form's fields; `id` is set when editing. */
type Draft = LlmConnectionInput & { id?: string; has_key?: boolean };

/** An empty form, starting from the first preset. */
function emptyDraft(): Draft {
  return { label: "", ...connectionPresets[0].input, api_key: "" };
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
  const [settings, setSettings] = useState<LlmSettings | null>(initial);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [template, setTemplate] = useState(initial?.template ?? "");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initial === null ? "Can't read the AI settings. Check that the note-taking server is running." : null
  );
  const templateId = useId();
  const autoId = useId();

  if (settings === null) {
    return (
      <div className="card max-w-2xl space-y-2">
        <h2 className="font-semibold">AI connections</h2>
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
      setMessage(llmErrorMessage(error));
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
      context_tokens: Number(draft.context_tokens),
      api_key: draft.api_key
    };
    const id = draft.id;
    // Claude Code and Codex have no address and never get a key (#105).
    if (isLocalCli(body.kind)) body.base_url = "";
    if (!body.api_key || isLocalCli(body.kind)) delete body.api_key;
    void run(async () => {
      const saved = await saveLlmConnection(body, id);
      // The first connection becomes the default, so the whole list is read again.
      const refreshed = await getLlmSettings().catch(() => null);
      setSettings((current) => refreshed ?? (current ? withConnection(current, saved) : current));
      setDraft(null);
      return `Saved ${saved.label}.`;
    });
  }

  const current = settings;
  return (
    <div className="card max-w-2xl space-y-4">
      <div className="space-y-1">
        <h2 className="font-semibold">AI connections</h2>
        <p className="text-sm text-muted">
          Summaries are written by a language model. A cloud service receives the meeting&apos;s transcript, notes and
          photos; a server on your local network keeps them in the office.
        </p>
      </div>

      {current.connections.length === 0 ? <p className="text-muted">No connection yet.</p> : null}
      <ul className="space-y-2">
        {current.connections.map((connection) => (
          <li key={connection.id} className="rounded-lg border border-line p-3">
            <div className="flex flex-wrap items-center gap-2">
              <span className="font-semibold">{connection.label}</span>
              {connection.is_default ? <span className="text-sm text-success">Default</span> : null}
              <span className="text-sm text-muted">
                {isLocalCli(connection.kind)
                  ? `${connection.kind === "codex" ? "Codex" : "Claude Code"} · ${connection.model || "default model"}`
                  : connection.model}
              </span>
            </div>
            {isLocalCli(connection.kind) ? (
              <p className="text-sm text-muted">
                {connection.tool_installed ? "Installed on the server's computer" : "Not found on the server's computer"}; runs with
                that computer&apos;s signed-in plan.
              </p>
            ) : (
              <>
                <p className="text-sm break-all text-muted">{connection.base_url}</p>
                {connection.plain_http ? (
                  <p className="text-sm text-warning">Unencrypted http: use it only on a network you trust.</p>
                ) : null}
                <p className="text-sm text-muted">{connection.has_key ? "API key saved" : "No API key"}</p>
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
                      return `${connection.label} is now the default.`;
                    })
                  }
                >
                  Make default
                </button>
              )}
              <button
                type="button"
                className={buttonClass}
                disabled={busy}
                onClick={() =>
                  void run(async () => `${connection.label} answered: ${await testLlmConnection(connection.id)}`)
                }
              >
                Test
              </button>
              <button
                type="button"
                className={buttonClass}
                disabled={busy}
                onClick={() => setDraft({ ...connection, api_key: "" })}
              >
                Edit
              </button>
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
                    return `Removed ${connection.label}.`;
                  })
                }
              >
                Remove
              </button>
            </div>
          </li>
        ))}
      </ul>

      {draft ? (
        <ConnectionForm draft={draft} busy={busy} onChange={setDraft} onSubmit={submit} onCancel={() => setDraft(null)} />
      ) : (
        <button type="button" className={buttonClass} onClick={() => setDraft(emptyDraft())}>
          Add connection
        </button>
      )}

      <div className="space-y-1">
        <label htmlFor={templateId} className="block font-semibold">
          Summary prompt
        </label>
        <p className="text-sm text-muted">
          Placeholders: {current.placeholders.map((name) => `{{${name}}}`).join(", ")}. The transcript is always sent
          after it.
        </p>
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
                return "Prompt saved.";
              })
            }
          >
            Save prompt
          </button>
          <button
            type="button"
            className={buttonClass}
            disabled={busy || !current.custom_template}
            onClick={() =>
              void run(async () => {
                const saved = await updateLlmSettings({ template: null });
                setSettings(saved);
                setTemplate(saved.template);
                return "Back to the built-in prompt.";
              })
            }
          >
            Reset to built-in
          </button>
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
              return enabled ? "Meetings will be summarized automatically." : "Automatic summaries are off.";
            });
          }}
        />
        <label htmlFor={autoId}>Summarize meetings automatically with the default connection</label>
      </div>

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
  const ids = { preset: useId(), label: useId(), kind: useId(), url: useId(), model: useId(), context: useId(), key: useId() };
  return (
    <form className="space-y-3 rounded-lg border border-line p-3" onSubmit={onSubmit}>
      <h3 className="font-semibold">{draft.id ? "Edit connection" : "New connection"}</h3>
      {draft.id ? null : (
        <div>
          <label htmlFor={ids.preset} className="block text-sm font-semibold">
            Start from
          </label>
          <select
            id={ids.preset}
            className={fieldClass}
            defaultValue={connectionPresets[0].name}
            onChange={(event) => {
              const preset = connectionPresets.find((candidate) => candidate.name === event.target.value);
              if (preset) onChange({ ...draft, ...preset.input });
            }}
          >
            {connectionPresets.map((preset) => (
              <option key={preset.name}>{preset.name}</option>
            ))}
          </select>
        </div>
      )}
      <div>
        <label htmlFor={ids.label} className="block text-sm font-semibold">
          Name
        </label>
        <input id={ids.label} className={fieldClass} required value={draft.label} onChange={(event) => onChange({ ...draft, label: event.target.value })} />
      </div>
      <div>
        <label htmlFor={ids.kind} className="block text-sm font-semibold">
          Protocol
        </label>
        <select id={ids.kind} className={fieldClass} value={draft.kind} onChange={(event) => onChange({ ...draft, kind: event.target.value as LlmKind })}>
          <option value="open_ai_compatible">OpenAI-compatible (OpenAI, Mistral, Ollama, LM Studio…)</option>
          <option value="anthropic">Anthropic</option>
          <option value="claude_code">Claude Code on the server&apos;s computer (Claude plan)</option>
          <option value="codex">Codex on the server&apos;s computer (ChatGPT plan)</option>
        </select>
      </div>
      {isLocalCli(draft.kind) ? (
        <p className="text-sm text-muted">
          Runs the tool installed and signed in on the computer running Note Taker, so that plan writes the summaries instead
          of API credit. The meeting goes to Anthropic or OpenAI under that account.
        </p>
      ) : (
        <div>
          <label htmlFor={ids.url} className="block text-sm font-semibold">
            Address
          </label>
          <input id={ids.url} className={fieldClass} required value={draft.base_url} onChange={(event) => onChange({ ...draft, base_url: event.target.value })} />
        </div>
      )}
      <div>
        <label htmlFor={ids.model} className="block text-sm font-semibold">
          Model
        </label>
        <input
          id={ids.model}
          className={fieldClass}
          required={!isLocalCli(draft.kind)}
          placeholder={isLocalCli(draft.kind) ? "Empty: the tool's own default" : undefined}
          value={draft.model}
          onChange={(event) => onChange({ ...draft, model: event.target.value })}
        />
      </div>
      <div>
        <label htmlFor={ids.context} className="block text-sm font-semibold">
          Context size (tokens)
        </label>
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
      {isLocalCli(draft.kind) ? null : (
        <div>
          <label htmlFor={ids.key} className="block text-sm font-semibold">
            API key
          </label>
          <input
            id={ids.key}
            type="password"
            autoComplete="off"
            className={fieldClass}
            placeholder={draft.has_key ? "Saved; type a new one to replace it" : "Not needed for most local servers"}
            value={draft.api_key ?? ""}
            onChange={(event) => onChange({ ...draft, api_key: event.target.value })}
          />
        </div>
      )}
      <div className="flex flex-wrap gap-2">
        <button type="submit" className={buttonClass} disabled={busy}>
          Save connection
        </button>
        <button type="button" className={buttonClass} onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}
