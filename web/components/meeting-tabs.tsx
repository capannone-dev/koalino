// The meeting page's tabs (#201): Summary · Transcript · Notes · Files, so a long meeting is not one
// endless page, and the Ask AI button that opens the chat about the meeting. The panels are
// rendered on the server and stay mounted while hidden, so a half-written note survives a switch.

"use client";

import { useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Icon } from "@/components/icons";
import { useT } from "@/lib/i18n/client";

/** The meeting page's tabs, in order. */
export const MEETING_TABS = ["summary", "transcript", "notes", "files"] as const;
export type MeetingTab = (typeof MEETING_TABS)[number];

/**
 * Shows a tab bar and the chosen tab's panel.
 *
 * Inputs: `initial`, the tab shown first; `panels`, the content of each tab.
 * Outputs: an ARIA tab list with its panels. Side effects: none beyond focus moving with the
 * arrow keys, Home and End.
 */
export function MeetingTabs({ initial, panels }: { initial: MeetingTab; panels: Record<MeetingTab, ReactNode> }) {
  const t = useT();
  const [selected, setSelected] = useState<MeetingTab>(initial);
  const baseId = useId();
  const tabRefs = useRef<Partial<Record<MeetingTab, HTMLButtonElement | null>>>({});

  /** Selects `tab` and moves the focus to it. */
  function choose(tab: MeetingTab) {
    setSelected(tab);
    tabRefs.current[tab]?.focus();
  }

  /** Arrow keys move to the previous or next tab, wrapping; Home and End to the ends. */
  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const last = MEETING_TABS.length - 1;
    const target =
      event.key === "ArrowRight" ? (index === last ? 0 : index + 1)
      : event.key === "ArrowLeft" ? (index === 0 ? last : index - 1)
      : event.key === "Home" ? 0
      : event.key === "End" ? last
      : null;
    if (target === null) return;
    event.preventDefault();
    choose(MEETING_TABS[target]);
  }

  return (
    <div className="mt-4">
      <div role="tablist" aria-label={t("meetingSections")} className="flex gap-1 overflow-x-auto border-b border-line">
        {MEETING_TABS.map((tab, index) => (
          <button
            key={tab}
            ref={(element) => {
              tabRefs.current[tab] = element;
            }}
            type="button"
            role="tab"
            id={`${baseId}-${tab}-tab`}
            aria-selected={selected === tab}
            aria-controls={`${baseId}-${tab}-panel`}
            tabIndex={selected === tab ? 0 : -1}
            onClick={() => setSelected(tab)}
            onKeyDown={(event) => onKeyDown(event, index)}
            className={`min-h-11 flex-1 whitespace-nowrap border-b-2 px-2 font-medium sm:flex-none sm:px-4 focus-visible:outline-2 focus-visible:outline-focus ${
              selected === tab ? "border-accent text-accent-text" : "border-transparent text-muted hover:text-ink"
            }`}
          >
            {t(tab)}
          </button>
        ))}
      </div>
      {MEETING_TABS.map((tab) => (
        <div
          key={tab}
          role="tabpanel"
          id={`${baseId}-${tab}-panel`}
          aria-labelledby={`${baseId}-${tab}-tab`}
          hidden={selected !== tab}
          className="pt-4"
        >
          {panels[tab]}
        </div>
      ))}
    </div>
  );
}

/**
 * The Ask AI button: shows the chat about the meeting (`children`) under it, or hides it again.
 *
 * Outputs: the button and, while open, the chat. Side effects: none.
 */
export function MeetingAskAi({ children }: { children: ReactNode }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const panelId = useId();
  return (
    <>
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen((value) => !value)}
        className="mt-3 inline-flex min-h-11 items-center gap-2 rounded-full border border-line-strong bg-accent-soft px-4 font-medium text-accent-text hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-focus"
      >
        <Icon name="sparkle" size={20} />
        {t("askAi")}
      </button>
      <div id={panelId}>{open ? children : null}</div>
    </>
  );
}
