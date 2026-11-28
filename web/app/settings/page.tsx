// Renders the web Settings page: the dashboard's language (#124), the entries to the Devices (Issue #84) and Telemetry (Issue #53)
// pages, the glossary of names and terms transcription spells as written, how long speech waits to
// be combined with another device's recording (#93), the daemon's log level
// and a download of its logs (Issue #46), and, with device sync on, where recordings are
// transcribed (Issue #59), the AI connections summaries are written with (#95), the opt-in
// notes folder on the server's computer (#98), whether online-meeting participants' voices
// are kept in the voice gallery (#103), the after-meeting pass and its model (#87), and whether
// meeting recordings are kept, with Wipe all recordings (#140).
//
// Layout (#209): the settings most people change first (language, devices, keeping recordings, AI
// connections), then Advanced, closed by default, grouped as Transcription, Privacy, Storage and
// Developer. Each setting saves as soon as it changes, except the glossary's free text.

import { AiSettings } from "@/components/ai-settings";
import { DevicesEntry } from "@/components/devices-entry";
import { GlossarySettings } from "@/components/glossary-settings";
import { LanguageSettings } from "@/components/language-settings";
import { LogSettings } from "@/components/log-settings";
import { MeetingVoicesSettings } from "@/components/meeting-voices-settings";
import { NotesFolderSettings } from "@/components/notes-folder-settings";
import { PassSettings } from "@/components/pass-settings";
import { RecordingsSettings } from "@/components/recordings-settings";
import { RetentionSettings } from "@/components/retention-settings";
import { TelemetryEntry } from "@/components/telemetry-entry";
import { TranscriptionRouteSettings } from "@/components/transcription-route-settings";
import { cookies } from "next/headers";
import type { ReactNode } from "react";
import { isLanguageCode, languageCookie } from "@/lib/i18n";
import { getGlossary, getLogLevel, getRetention } from "@/lib/api";
import { getLlmSettings } from "@/lib/llm-api";
import { getMeetingVoices } from "@/lib/meeting-voices-api";
import { getNotesFolder } from "@/lib/notes-folder-api";
import { getAfterMeetingPass } from "@/lib/pass-api";
import { getP2pStatus } from "@/lib/p2p-api";
import { getRecordingsStatus } from "@/lib/recordings-api";
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
  const recordings = await getRecordingsStatus().catch(() => null);
  const savedLanguage = (await cookies()).get(languageCookie)?.value;
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">{t("settings")}</h1>
      <LanguageSettings initial={isLanguageCode(savedLanguage) ? savedLanguage : null} />
      <DevicesEntry />
      <RecordingsSettings initial={recordings} />
      <AiSettings initial={llmSettings} />
      <details className="group max-w-2xl rounded-xl border border-line bg-surface">
        <summary className="flex min-h-11 cursor-pointer items-center justify-between px-4 py-3 font-semibold focus-visible:outline-2 focus-visible:outline-focus">
          <span>
            {t("advancedSettings")}
            <span className="block text-sm font-normal text-muted">{t("advancedSettingsDescription")}</span>
          </span>
          <span aria-hidden="true" className="transition-transform group-open:rotate-180">
            ▾
          </span>
        </summary>
        <div className="space-y-4 border-t border-line px-4 pb-4">
          <SettingsGroup title={t("transcription")}>
            {p2pStatus?.enabled ? (
              <TranscriptionRouteSettings
                initialRoute={p2pStatus.transcription_route}
                devices={p2pStatus.trusted_devices.map((device) => ({ peerId: device.peer_id, name: device.device_name }))}
              />
            ) : null}
            <GlossarySettings initialTerms={glossary} />
            <PassSettings initial={afterMeetingPass} />
            <RetentionSettings initial={retention} />
          </SettingsGroup>
          <SettingsGroup title={t("privacy")}>
            <MeetingVoicesSettings initial={meetingVoices} />
          </SettingsGroup>
          <SettingsGroup title={t("storage")}>
            <NotesFolderSettings initial={notesFolder} />
          </SettingsGroup>
          <SettingsGroup title={t("developer")}>
            <TelemetryEntry />
            <LogSettings initialLevel={level} />
          </SettingsGroup>
        </div>
      </details>
    </section>
  );
}

/** A titled group of settings on the Advanced part of the page (#209). */
function SettingsGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="space-y-4 pt-4">
      <h2 className="text-sm font-semibold uppercase tracking-wide text-accent-text">{title}</h2>
      {children}
    </section>
  );
}
