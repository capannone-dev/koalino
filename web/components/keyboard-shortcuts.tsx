// The dashboard's keyboard shortcuts (#215), on every page: Ctrl/Cmd+K opens the command palette
// (go to a page, see the shortcuts), "/" puts the cursor in the page's search field, and "?" lists
// the shortcuts. The meeting player adds Space, J and L (meeting-recording-player.tsx). Single keys
// never fire while typing in a field.

"use client";

import { useRouter } from "next/navigation";
import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { Icon, type IconName } from "@/components/icons";
import { useT } from "@/lib/i18n/client";
import { hasCommandModifier, isTypingTarget } from "@/lib/shortcut-keys";

/** One thing the palette can do. */
type Command = { label: string; icon: IconName; keys?: string; run: () => void };

/** Which of the two dialogs is open. */
type Open = "palette" | "help" | null;

/**
 * Listens for the shortcuts and shows the palette or the shortcut list.
 *
 * Outputs: nothing until a dialog opens. Side effects: a document key listener; the palette
 * navigates between pages.
 */
export function KeyboardShortcuts() {
  const t = useT();
  const router = useRouter();
  const [open, setOpen] = useState<Open>(null);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setOpen((current) => (current === "palette" ? null : "palette"));
        return;
      }
      if (event.key === "Escape") {
        setOpen(null);
        return;
      }
      if (hasCommandModifier(event) || isTypingTarget(event.target)) return;
      if (event.key === "?") {
        event.preventDefault();
        setOpen("help");
      } else if (event.key === "/") {
        const field = document.querySelector<HTMLInputElement>("[data-shortcut-search]");
        if (field) {
          event.preventDefault();
          field.focus();
        }
      }
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);

  const commands = useMemo<Command[]>(() => {
    const go = (path: string) => () => router.push(path);
    return [
      { label: t("goToPlace", { place: t("meetings") }), icon: "meetings", run: go("/meetings") },
      { label: t("goToPlace", { place: t("projects") }), icon: "projects", run: go("/projects") },
      { label: t("goToPlace", { place: t("chat") }), icon: "chat", run: go("/chat") },
      { label: t("goToPlace", { place: t("people") }), icon: "people", run: go("/voice-gallery") },
      { label: t("goToPlace", { place: t("devices") }), icon: "devices", run: go("/devices") },
      { label: t("goToPlace", { place: t("settings") }), icon: "settings", run: go("/settings") },
      { label: t("keyboardShortcuts"), icon: "more", keys: "?", run: () => setOpen("help") }
    ];
  }, [router, t]);

  if (open === "palette") {
    return (
      <Palette
        commands={commands}
        onClose={() => setOpen(null)}
        onRun={(command) => {
          setOpen(null);
          command.run();
        }}
      />
    );
  }
  if (open === "help") return <ShortcutList onClose={() => setOpen(null)} />;
  return null;
}

/** The dimmed backdrop and centred panel both dialogs use; a click outside closes it. */
function Overlay({ label, onClose, children }: { label: string; onClose: () => void; children: ReactNode }) {
  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-slate-900/50 p-4 pt-[15vh]" onClick={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={label}
        className="w-full max-w-lg rounded-xl border border-line bg-surface p-3 shadow-xl"
        onClick={(event) => event.stopPropagation()}
      >
        {children}
      </div>
    </div>
  );
}

/** The command palette: a field that filters the commands; Enter runs the first match. */
function Palette({ commands, onClose, onRun }: { commands: Command[]; onClose: () => void; onRun: (command: Command) => void }) {
  const t = useT();
  const [query, setQuery] = useState("");
  const field = useRef<HTMLInputElement>(null);
  const listId = useId();
  useEffect(() => field.current?.focus(), []);
  const matches = commands.filter((command) => command.label.toLowerCase().includes(query.trim().toLowerCase()));
  return (
    <Overlay label={t("commandPalette")} onClose={onClose}>
      <input
        ref={field}
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls={listId}
        aria-label={t("typeACommand")}
        placeholder={t("typeACommand")}
        className="min-h-11 w-full rounded-lg border border-line-strong bg-surface px-3"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && matches[0]) {
            event.preventDefault();
            onRun(matches[0]);
          }
        }}
      />
      <ul id={listId} role="listbox" className="mt-2 max-h-80 overflow-y-auto">
        {matches.length === 0 ? <li className="p-3 text-muted">{t("noMatchingCommands")}</li> : null}
        {matches.map((command) => (
          <li key={command.label} role="option" aria-selected="false">
            <button
              type="button"
              className="flex min-h-11 w-full items-center gap-3 rounded-lg px-3 text-left hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus"
              onClick={() => onRun(command)}
            >
              <Icon name={command.icon} size={20} />
              <span className="flex-1">{command.label}</span>
              {command.keys ? <kbd className="rounded border border-line px-1.5 text-xs text-muted">{command.keys}</kbd> : null}
            </button>
          </li>
        ))}
      </ul>
    </Overlay>
  );
}

/** The list of shortcuts ("?"). */
function ShortcutList({ onClose }: { onClose: () => void }) {
  const t = useT();
  const close = useRef<HTMLButtonElement>(null);
  useEffect(() => close.current?.focus(), []);
  const rows: [string, string][] = [
    ["Ctrl/⌘ K", t("openTheCommandPalette")],
    ["/", t("searchThisPage")],
    [t("spaceKey"), t("playOrPauseTheRecording")],
    ["J  L", t("back10SecondsForward10Seconds")],
    ["?", t("showTheseShortcuts")]
  ];
  return (
    <Overlay label={t("keyboardShortcuts")} onClose={onClose}>
      <h2 className="px-1 pb-2 font-semibold">{t("keyboardShortcuts")}</h2>
      <dl className="space-y-2 px-1">
        {rows.map(([keys, what]) => (
          <div key={keys} className="flex items-center gap-3">
            <dt className="w-24">
              <kbd className="rounded border border-line px-1.5 text-sm">{keys}</kbd>
            </dt>
            <dd>{what}</dd>
          </div>
        ))}
      </dl>
      <div className="mt-3 flex justify-end">
        <button ref={close} type="button" className="min-h-11 rounded-lg px-4 font-medium text-accent-text hover:bg-surface-muted" onClick={onClose}>
          {t("close")}
        </button>
      </div>
    </Overlay>
  );
}
