// Plays a meeting's kept recording (#140) on its page, shown only when the daemon keeps one.

"use client";

import { useEffect, useState } from "react";
import { hasMeetingRecording, meetingRecordingUrl } from "@/lib/recordings-api";
import { useT } from "@/lib/i18n/client";

/**
 * Shows an audio player for the meeting's recording.
 *
 * Inputs: the meeting. Outputs: nothing until the daemon says a recording is kept, then a heading
 * and an `<audio>` player that loads the WAV only when played. Side effects: asks the daemon once.
 */
export function MeetingRecordingPlayer({ meetingId }: { meetingId: string }) {
  const t = useT();
  const [available, setAvailable] = useState(false);

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

  if (!available) return null;
  return (
    <section aria-label={t("playRecording")} className="my-3 space-y-1">
      <h2>{t("playRecording")}</h2>
      <audio controls preload="none" src={meetingRecordingUrl(meetingId)} className="w-full max-w-2xl">
        {t("recordingCouldNotBePlayed")}
      </audio>
    </section>
  );
}
