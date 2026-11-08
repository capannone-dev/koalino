// Renders the web Settings page: the entries to the Devices (Issue #84) and Telemetry (Issue #53)
// pages, the glossary of names and terms transcription spells as written, the daemon's log level
// and a download of its logs (Issue #46), and, with device sync on, where recordings are
// transcribed (Issue #59).

import { DevicesEntry } from "@/components/devices-entry";
import { GlossarySettings } from "@/components/glossary-settings";
import { LogSettings } from "@/components/log-settings";
import { TelemetryEntry } from "@/components/telemetry-entry";
import { TranscriptionRouteSettings } from "@/components/transcription-route-settings";
import { getGlossary, getLogLevel } from "@/lib/api";
import { getP2pStatus } from "@/lib/p2p-api";

export default async function SettingsPage() {
  const level = await getLogLevel().catch(() => null);
  const glossary = await getGlossary().catch(() => null);
  const p2pStatus = await getP2pStatus().catch(() => null);
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">Settings</h1>
      <DevicesEntry />
      {p2pStatus?.enabled ? (
        <TranscriptionRouteSettings
          initialRoute={p2pStatus.transcription_route}
          devices={p2pStatus.trusted_devices.map((device) => ({ peerId: device.peer_id, name: device.device_name }))}
        />
      ) : null}
      <GlossarySettings initialTerms={glossary} />
      <TelemetryEntry />
      <LogSettings initialLevel={level} />
    </section>
  );
}
