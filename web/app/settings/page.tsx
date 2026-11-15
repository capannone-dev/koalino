// Renders the web Settings page: the dashboard's language (#124), the entries to the Devices (Issue #84) and Telemetry (Issue #53)
// pages, the glossary of names and terms transcription spells as written, how long speech waits to
// be combined with another device's recording (#93), the daemon's log level
// and a download of its logs (Issue #46), and, with device sync on, where recordings are
// transcribed (Issue #59), the AI connections summaries are written with (#95), the opt-in
// notes folder on the server's computer (#98), whether online-meeting participants' voices
// are kept in the voice gallery (#103), and the after-meeting pass and its model (#87).

import { AiSettings } from "@/components/ai-settings";
import { DevicesEntry } from "@/components/devices-entry";
import { GlossarySettings } from "@/components/glossary-settings";
import { LanguageSettings } from "@/components/language-settings";
import { LogSettings } from "@/components/log-settings";
import { MeetingVoicesSettings } from "@/components/meeting-voices-settings";
import { NotesFolderSettings } from "@/components/notes-folder-settings";
import { PassSettings } from "@/components/pass-settings";
import { RetentionSettings } from "@/components/retention-settings";
import { TelemetryEntry } from "@/components/telemetry-entry";
import { TranscriptionRouteSettings } from "@/components/transcription-route-settings";
import { cookies } from "next/headers";
import { isLanguageCode, languageCookie } from "@/lib/i18n";
import { getGlossary, getLogLevel, getRetention } from "@/lib/api";
import { getLlmSettings } from "@/lib/llm-api";
import { getMeetingVoices } from "@/lib/meeting-voices-api";
import { getNotesFolder } from "@/lib/notes-folder-api";
import { getAfterMeetingPass } from "@/lib/pass-api";
import { getP2pStatus } from "@/lib/p2p-api";
import { getT } from "@/lib/i18n/server";

export default async function SettingsPage() {
  const t = await getT();
  const level = await getLogLevel().catch(() => null);
  const glossary = await getGlossary().catch(() => null);
  const retention = await getRetention().catch(() => null);
  const afterMeetingPass = await getAfterMeetingPass().catch(() => null);
  const p2pStatus = await getP2pStatus().catch(() => null);
  const llmSettings = await getLlmSettings().catch(() => null);
  const notesFolder = await getNotesFolder().catch(() => null);
  const meetingVoices = await getMeetingVoices().catch(() => null);
  const savedLanguage = (await cookies()).get(languageCookie)?.value;
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">{t("settings")}</h1>
      <LanguageSettings initial={isLanguageCode(savedLanguage) ? savedLanguage : null} />
      <DevicesEntry />
      {p2pStatus?.enabled ? (
        <TranscriptionRouteSettings
          initialRoute={p2pStatus.transcription_route}
          devices={p2pStatus.trusted_devices.map((device) => ({ peerId: device.peer_id, name: device.device_name }))}
        />
      ) : null}
      <GlossarySettings initialTerms={glossary} />
      <PassSettings initial={afterMeetingPass} />
      <RetentionSettings initial={retention} />
      <MeetingVoicesSettings initial={meetingVoices} />
      <AiSettings initial={llmSettings} />
      <NotesFolderSettings initial={notesFolder} />
      <TelemetryEntry />
      <LogSettings initialLevel={level} />
    </section>
  );
}
