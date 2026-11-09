// Renders the web Settings page: the entries to the Devices (Issue #84) and Telemetry (Issue #53)
// pages, the glossary of names and terms transcription spells as written, how long speech waits to
// be combined with another device's recording (#93), the daemon's log level
// and a download of its logs (Issue #46), and, with device sync on, where recordings are
// transcribed (Issue #59), the AI connections summaries are written with (#95), and the opt-in
// notes folder on the server's computer (#98).

import { AiSettings } from "@/components/ai-settings";
import { DevicesEntry } from "@/components/devices-entry";
import { GlossarySettings } from "@/components/glossary-settings";
import { LogSettings } from "@/components/log-settings";
import { NotesFolderSettings } from "@/components/notes-folder-settings";
import { RetentionSettings } from "@/components/retention-settings";
import { TelemetryEntry } from "@/components/telemetry-entry";
import { TranscriptionRouteSettings } from "@/components/transcription-route-settings";
import { getGlossary, getLogLevel, getRetention } from "@/lib/api";
import { getLlmSettings } from "@/lib/llm-api";
import { getNotesFolder } from "@/lib/notes-folder-api";
import { getP2pStatus } from "@/lib/p2p-api";

export default async function SettingsPage() {
  const level = await getLogLevel().catch(() => null);
  const glossary = await getGlossary().catch(() => null);
  const retention = await getRetention().catch(() => null);
  const p2pStatus = await getP2pStatus().catch(() => null);
  const llmSettings = await getLlmSettings().catch(() => null);
  const notesFolder = await getNotesFolder().catch(() => null);
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
      <RetentionSettings initial={retention} />
      <AiSettings initial={llmSettings} />
      <NotesFolderSettings initial={notesFolder} />
      <TelemetryEntry />
      <LogSettings initialLevel={level} />
    </section>
  );
}
