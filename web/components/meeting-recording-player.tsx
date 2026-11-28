// The meeting page's player for a kept recording (#140, #197): a bar that stays at the bottom of
// the window while the transcript scrolls, with play and pause, back and forward 10 seconds, the
// position, the speed, a button to follow the transcript again after scrolling away, and a link
// that downloads the recording (#199). The playback itself is shared with the transcript through
// `RecordingPlaybackProvider`.

"use client";

import { PLAYBACK_RATES, SKIP_MS, recordingFileName, useRecordingPlayback } from "@/lib/recording-playback";
import { useT } from "@/lib/i18n/client";
import { hasCommandModifier, isTypingTarget } from "@/lib/shortcut-keys";
import { useEffect, useRef } from "react";

const iconButton =
  "inline-flex min-h-11 min-w-11 items-center justify-center rounded-full border border-line-strong hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus";

/** Formats milliseconds as `m:ss`, or `h:mm:ss` past an hour. */
export function formatPlaybackTime(milliseconds: number): string {
  const totalSeconds = Math.floor(Math.max(0, milliseconds) / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}` : `${minutes}:${seconds}`;
}

/**
 * Space plays or pauses, J and L go back or forward 10 seconds (#215), while a recording can be
 * played and the owner is not typing or on a button.
 *
 * Side effects: a document key listener while mounted.
 */
function usePlayerKeys(playback: ReturnType<typeof useRecordingPlayback>) {
  const latest = useRef(playback);
  latest.current = playback;
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const current = latest.current;
      if (!current?.available || hasCommandModifier(event) || isTypingTarget(event.target)) return;
      const key = event.key.toLowerCase();
      if (key === " ") current.toggle();
      else if (key === "j") current.skip(-SKIP_MS);
      else if (key === "l") current.skip(SKIP_MS);
      else return;
      event.preventDefault();
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, []);
}

/** Words a speed as `1.25×`. */
export function formatRate(rate: number): string {
  return `${rate}×`;
}

/**
 * Shows the player bar, or nothing when the meeting has no kept recording.
 *
 * Inputs: `title`, the meeting's title, which names the downloaded recording; the name is made
 * here, on the client, because the server page cannot call this client module's helpers (#200).
 * The rest is the shared playback.
 * Outputs: the bar. Side effects: plays the recording; the link downloads it.
 */
export function MeetingRecordingPlayer({ title }: { title: string | null | undefined }) {
  const t = useT();
  const playback = useRecordingPlayback();
  usePlayerKeys(playback);
  if (!playback?.available) return null;
  const { playing, failed, positionMs, durationMs, rate, follow } = playback;
  return (
    <section
      aria-label={t("recordingPlayer")}
      className="sticky bottom-0 z-10 my-3 flex flex-wrap items-center gap-2 border-t border-line bg-canvas py-2"
    >
      <button type="button" className={iconButton} aria-label={t("skipBack10Seconds")} title={t("skipBack10Seconds")} onClick={() => playback.skip(-SKIP_MS)}>
        −10
      </button>
      <button
        type="button"
        className={`${iconButton} bg-accent text-on-accent`}
        aria-label={playing ? t("pause") : t("play")}
        title={playing ? t("pause") : t("play")}
        onClick={playback.toggle}
      >
        {playing ? (
          <svg aria-hidden="true" viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
            <path d="M7 5h4v14H7zM13 5h4v14h-4z" />
          </svg>
        ) : (
          <svg aria-hidden="true" viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
            <path d="M8 5v14l11-7z" />
          </svg>
        )}
      </button>
      <button type="button" className={iconButton} aria-label={t("skipForward10Seconds")} title={t("skipForward10Seconds")} onClick={() => playback.skip(SKIP_MS)}>
        +10
      </button>
      <input
        type="range"
        className="min-w-40 flex-1"
        aria-label={t("positionInTheRecording")}
        aria-valuetext={formatPlaybackTime(positionMs)}
        min={0}
        max={Math.max(durationMs, 1)}
        step={1000}
        value={Math.min(positionMs, Math.max(durationMs, 1))}
        disabled={durationMs === 0}
        onChange={(event) => playback.seekTo(Number(event.target.value))}
      />
      <span className="tabular-nums text-sm text-muted">
        {failed ? t("recordingCouldNotBePlayed") : `${formatPlaybackTime(positionMs)} / ${formatPlaybackTime(durationMs)}`}
      </span>
      <label className="flex items-center gap-1 text-sm">
        <span className="sr-only">{t("playbackSpeed")}</span>
        <select
          className="min-h-11 rounded-lg border border-line-strong bg-surface px-2"
          title={t("playbackSpeed")}
          value={rate}
          onChange={(event) => playback.setRate(Number(event.target.value))}
        >
          {PLAYBACK_RATES.map((option) => (
            <option key={option} value={option}>
              {formatRate(option)}
            </option>
          ))}
        </select>
      </label>
      <a
        href={playback.url}
        download={recordingFileName(title)}
        className={iconButton}
        aria-label={t("downloadTheRecording")}
        title={t("downloadTheRecording")}
      >
        <svg aria-hidden="true" viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
          <path d="M11 4h2v9.17l3.59-3.58L18 11l-6 6-6-6 1.41-1.41L11 13.17zM5 18h14v2H5z" />
        </svg>
      </a>
      {playing && !follow ? (
        <button type="button" className={`${iconButton} px-3`} onClick={() => playback.setFollow(true)}>
          {t("followTranscript")}
        </button>
      ) : null}
    </section>
  );
}
