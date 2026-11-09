/**
 * Collects what Teams shows during a meeting, as it changes, into timed cues:
 *
 * - `CaptionCollector`: the live captions. Each caption element grows word by word and is then
 *   punctuated; Teams' virtual list may later reuse the element for another caption. A cue keeps
 *   the time it was first seen and is updated while its words keep the same beginning.
 * - `TranscriptPanelCollector`: the transcript panel shown while Teams transcription runs. Its
 *   items have stable IDs, so each ID is one cue.
 *
 * Times are milliseconds from the meeting start, as the content script observed them (captions
 * appear about a second after the words are spoken). Text is never logged.
 */
import { VttCue, buildVtt } from "./vtt";

/** One caption as read from the page. */
export interface CaptionSnapshot {
  /** The caption's element (or any object standing for it): its identity follows the caption. */
  key: object;
  /** The speaker's display name. */
  speaker: string;
  /** The speaker's Teams ID (`8:orgid:…`), when the page shows it. */
  speakerId?: string;
  /** The caption's current words. */
  text: string;
}

/** A caption cue, with the speaker's Teams ID when known. */
export interface CaptionCue extends VttCue {
  speakerId?: string;
}

/** Normalized beginning used to tell a growing caption from a reused element. */
function beginning(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\p{L}\p{N} ]/gu, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 12);
}

/** Whether `next` continues the caption that showed `previous`. */
function continues(previous: string, next: string): boolean {
  const before = beginning(previous);
  const after = beginning(next);
  const shorter = Math.min(before.length, after.length);
  return shorter === 0 || before.slice(0, shorter) === after.slice(0, shorter);
}

/** Follows live captions into cues. */
export class CaptionCollector {
  private readonly current = new WeakMap<object, CaptionCue>();
  private readonly active: CaptionCue[] = [];

  /**
   * Takes in the captions shown now.
   *
   * Inputs: Every caption currently on screen and the meeting time.
   * Outputs: None. Side effects: Creates or updates cues.
   */
  observe(snapshots: readonly CaptionSnapshot[], nowMs: number): void {
    for (const snapshot of snapshots) {
      const text = snapshot.text.replace(/\s+/g, " ").trim();
      if (text === "") continue;
      const existing = this.current.get(snapshot.key);
      if (existing && existing.speaker === snapshot.speaker && continues(existing.text, text)) {
        existing.text = text;
        existing.endMs = nowMs;
        continue;
      }
      const cue: CaptionCue = {
        startMs: nowMs,
        endMs: nowMs,
        speaker: snapshot.speaker,
        ...(snapshot.speakerId ? { speakerId: snapshot.speakerId } : {}),
        text,
      };
      this.current.set(snapshot.key, cue);
      this.active.push(cue);
    }
  }

  /** The cues so far, in time order. Side effects: None. */
  cues(): CaptionCue[] {
    return [...this.active].sort((first, second) => first.startMs - second.startMs);
  }

  /** Whether nothing was captioned. Side effects: None. */
  isEmpty(): boolean {
    return this.active.length === 0;
  }

  /** The cues as WebVTT. Side effects: None. */
  toVtt(): string {
    return buildVtt(this.cues());
  }
}

/** One item of the Teams transcript panel. */
export interface TranscriptPanelItem {
  /** The item's stable ID (from its `transcript-chat-message-…` data-tid). */
  id: string;
  speaker: string;
  text: string;
}

/** Follows the Teams transcript panel into cues. */
export class TranscriptPanelCollector {
  private readonly byId = new Map<string, VttCue>();

  /**
   * Takes in the panel items shown now.
   *
   * Inputs: The items and the meeting time. Side effects: Creates or updates cues.
   */
  observe(items: readonly TranscriptPanelItem[], nowMs: number): void {
    for (const item of items) {
      const text = item.text.replace(/\s+/g, " ").trim();
      if (text === "") continue;
      const existing = this.byId.get(item.id);
      if (existing) {
        if (existing.text !== text) {
          existing.text = text;
          existing.endMs = nowMs;
        }
        continue;
      }
      this.byId.set(item.id, { startMs: nowMs, endMs: nowMs, speaker: item.speaker, text });
    }
  }

  /** The cues in time order. Side effects: None. */
  cues(): VttCue[] {
    return [...this.byId.values()].sort((first, second) => first.startMs - second.startMs);
  }

  /** Whether nothing was collected. Side effects: None. */
  isEmpty(): boolean {
    return this.byId.size === 0;
  }

  /** The cues as WebVTT. Side effects: None. */
  toVtt(): string {
    return buildVtt(this.cues());
  }
}
