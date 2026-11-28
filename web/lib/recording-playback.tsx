// Shares one meeting's kept-recording playback (#197) between the player bar and the transcript,
// so the transcript marks the passage being heard, keeps it in view, and plays any passage from its
// start. The recording is a WAV on the meeting's timeline (silence between speech), so a passage's
// start time is its place in the audio. The `<audio>` element loads it only when first played.

"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { hasMeetingRecording, meetingRecordingUrl } from "@/lib/recordings-api";

/** The speeds offered, as multiples of the normal speed. */
export const PLAYBACK_RATES = [0.75, 1, 1.25, 1.5, 1.75, 2] as const;

/** How far the skip buttons move, in milliseconds. */
export const SKIP_MS = 10_000;

/** What the player bar and the transcript share. */
export type RecordingPlayback = {
  /** Whether the meeting has a kept recording. */
  available: boolean;
  /** Where the recording is served, for downloading it (#199). */
  url: string;
  playing: boolean;
  /** Whether the last attempt to play failed. */
  failed: boolean;
  positionMs: number;
  /** The recording's length, or 0 until it has loaded. */
  durationMs: number;
  rate: number;
  /** Whether the transcript keeps the passage being heard in view. */
  follow: boolean;
  toggle: () => void;
  seekTo: (positionMs: number) => void;
  skip: (deltaMs: number) => void;
  playFrom: (positionMs: number) => void;
  setRate: (rate: number) => void;
  setFollow: (follow: boolean) => void;
};

const PlaybackContext = createContext<RecordingPlayback | null>(null);

/** Returns the shared playback, or `null` outside a `RecordingPlaybackProvider`. */
export function useRecordingPlayback(): RecordingPlayback | null {
  return useContext(PlaybackContext);
}

/**
 * Names a downloaded recording after its meeting: `<title>.wav`, with characters file systems
 * reject replaced by `_`, or `meeting.wav` without a title (#199). Matches the app's names.
 */
export function recordingFileName(title: string | null | undefined): string {
  const safeTitle = (title ?? "").replace(/[\\/:*?"<>|\x00-\x1F]/g, "_").trim();
  return `${safeTitle || "meeting"}.wav`;
}

/** Returns the start of the passage being heard: the last start at or before `positionMs`. */
export function passageStartAt(starts: number[], positionMs: number): number | null {
  let current: number | null = null;
  for (const start of starts) {
    if (start > positionMs) break;
    current = start;
  }
  return current;
}

/**
 * Provides the playback of `meetingId`'s kept recording to its children.
 *
 * Inputs: the meeting and the page parts that use it. Outputs: the children and a hidden `<audio>`
 * element. Side effects: asks the daemon once whether a recording is kept; streams it when played.
 */
export function RecordingPlaybackProvider({ meetingId, children }: { meetingId: string; children: ReactNode }) {
  const audio = useRef<HTMLAudioElement | null>(null);
  const [available, setAvailable] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [failed, setFailed] = useState(false);
  const [positionMs, setPositionMs] = useState(0);
  const [durationMs, setDurationMs] = useState(0);
  const [rate, setRateState] = useState(1);
  const [follow, setFollow] = useState(true);

  useEffect(() => {
    let current = true;
    hasMeetingRecording(meetingId)
      .then((has) => {
        if (current) setAvailable(has);
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [meetingId]);

  const play = useCallback(() => {
    const element = audio.current;
    if (!element) return;
    setFailed(false);
    element.playbackRate = rate;
    element.play().catch(() => setFailed(true));
  }, [rate]);

  const seekTo = useCallback(
    (target: number) => {
      const element = audio.current;
      if (!element) return;
      const end = durationMs > 0 ? durationMs : Number.POSITIVE_INFINITY;
      const clamped = Math.min(Math.max(0, target), end);
      element.currentTime = clamped / 1000;
      setPositionMs(clamped);
    },
    [durationMs]
  );

  const value = useMemo<RecordingPlayback>(
    () => ({
      available,
      url: meetingRecordingUrl(meetingId),
      playing,
      failed,
      positionMs,
      durationMs,
      rate,
      follow,
      toggle: () => {
        const element = audio.current;
        if (!element) return;
        if (element.paused) play();
        else element.pause();
      },
      seekTo,
      skip: (deltaMs) => seekTo(positionMs + deltaMs),
      playFrom: (target) => {
        setFollow(true);
        seekTo(target);
        if (audio.current?.paused) play();
      },
      setRate: (next) => {
        setRateState(next);
        if (audio.current) audio.current.playbackRate = next;
      },
      setFollow
    }),
    [available, meetingId, playing, failed, positionMs, durationMs, rate, follow, play, seekTo]
  );

  return (
    <PlaybackContext.Provider value={value}>
      {children}
      {available ? (
        <audio
          ref={audio}
          preload="none"
          src={meetingRecordingUrl(meetingId)}
          onPlay={() => setPlaying(true)}
          onPause={() => setPlaying(false)}
          onEnded={() => setPlaying(false)}
          onError={() => setFailed(true)}
          onTimeUpdate={(event) => setPositionMs(Math.round(event.currentTarget.currentTime * 1000))}
          onLoadedMetadata={(event) => {
            const seconds = event.currentTarget.duration;
            if (Number.isFinite(seconds)) setDurationMs(Math.round(seconds * 1000));
          }}
        />
      ) : null}
    </PlaybackContext.Provider>
  );
}
