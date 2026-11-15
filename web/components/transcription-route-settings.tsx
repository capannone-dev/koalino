// Renders the web Settings page's choice of where recordings are transcribed (Issue #59). The
// daemon keeps and applies the choice; this offers this device, the best device and paired devices.

"use client";

import { ChangeEvent, useId, useState } from "react";
import { setTranscriptionRoute } from "@/lib/p2p-api";
import { useT } from "@/lib/i18n/client";

export type TranscriptionRouteSettingsProps = {
  /** The route in effect: `local`, `auto`, or a device's peer ID. */
  initialRoute: string;
  /** The paired devices. */
  devices: { peerId: string; name: string }[];
};

/**
 * Lets the owner choose where recordings are transcribed.
 *
 * Inputs: see `TranscriptionRouteSettingsProps`. Outputs: a selector and a status line (announced
 * to screen readers). Side effects: `PUT /api/p2p/transcription-route` on a change; a refusal keeps
 * the old choice.
 */
export function TranscriptionRouteSettings({ initialRoute, devices }: TranscriptionRouteSettingsProps) {
  const t = useT();
  const [route, setRoute] = useState(initialRoute);
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const selectId = useId();
  const isRemoved = route !== "local" && route !== "auto" && !devices.some((device) => device.peerId === route);

  /** Applies the chosen route, keeping the old one when the daemon refuses it. */
  async function changeRoute(event: ChangeEvent<HTMLSelectElement>) {
    const chosen = event.target.value;
    setIsSaving(true);
    setMessage(null);
    try {
      await setTranscriptionRoute(chosen);
      setRoute(chosen);
    } catch {
      setMessage(t("theTranscriptionDeviceCouldNot"));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="card max-w-2xl space-y-2">
      <label htmlFor={selectId} className="block font-semibold">{t("transcribeOn")}</label>
      <p className="text-sm text-muted">{t("anotherDeviceReceivesTheRecordings")}</p>
      <select id={selectId} value={route} disabled={isSaving} onChange={changeRoute}>
        <option value="local">{t("thisDevice")}</option>
        <option value="auto">{t("bestAvailable")}</option>
        {devices.map((device) => (
          <option key={device.peerId} value={device.peerId}>
            {device.name}
          </option>
        ))}
        {isRemoved ? <option value={route}>{t("aRemovedDevice")}</option> : null}
      </select>
      <p role="status" className="text-sm">
        {message}
      </p>
    </div>
  );
}
