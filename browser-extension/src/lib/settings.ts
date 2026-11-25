/**
 * The extension's settings (kept in `chrome.storage.local`, per browser profile), with their
 * defaults. Every capture is off until the owner agrees to record a meeting; starting Teams
 * transcription is a separate opt-in because it is visible to everyone and stores the transcript
 * in Microsoft's cloud.
 *
 * Per-site choices (#174) are kept by site key (`siteKey`: a service such as `meet`, or a host):
 * record without asking, or never ask. Teams keeps its own "Always for Teams" switch.
 */

/** The owner's choices. */
export interface ExtensionSettings {
  /** Record every Teams meeting without asking ("Always for Teams"). */
  alwaysRecordTeams: boolean;
  /** When recording, start Teams' own transcription if the meeting allows it. */
  startTeamsTranscription: boolean;
  /** Keep screen-share slides. */
  captureSlides: boolean;
  /** Turn on and keep Teams live captions (speaker names and timing). */
  captureCaptions: boolean;
  /** Send the Teams transcript to Koalino when Teams publishes it. */
  fetchTeamsTranscripts: boolean;
  /**
   * When recording, set the meeting's spoken language in Teams to Koalino's fixed language
   * (#141). Off by default: Teams applies it to the whole meeting.
   */
  matchTeamsSpokenLanguage: boolean;
  /** Sites recorded without asking when a call starts (#174). */
  alwaysRecordSites: string[];
  /** Sites where the extension never asks; "Record this tab" still works there (#174). */
  neverAskSites: string[];
  /**
   * Watch every website for calls, not only the known meeting sites (#174). Needs the owner to
   * allow access to all sites; off by default.
   */
  watchAllSites: boolean;
}

/** The settings before the owner changes anything. */
export const DEFAULT_SETTINGS: ExtensionSettings = {
  alwaysRecordTeams: false,
  startTeamsTranscription: false,
  captureSlides: true,
  captureCaptions: true,
  fetchTeamsTranscripts: true,
  matchTeamsSpokenLanguage: false,
  alwaysRecordSites: [],
  neverAskSites: [],
  watchAllSites: false,
};

/** The settings kept as lists of site keys. */
const SITE_LISTS = ["alwaysRecordSites", "neverAskSites"] as const;

/**
 * Reads stored settings, keeping only known values of the right type.
 *
 * Inputs: Whatever storage returned. Outputs: Complete settings; a site list keeps only its
 * strings. Side effects: None.
 */
export function normalizeSettings(stored: unknown): ExtensionSettings {
  const settings: ExtensionSettings = { ...DEFAULT_SETTINGS, alwaysRecordSites: [], neverAskSites: [] };
  if (typeof stored !== "object" || stored === null) return settings;
  const values = stored as Record<string, unknown>;
  for (const key of Object.keys(DEFAULT_SETTINGS) as (keyof ExtensionSettings)[]) {
    const value = values[key];
    if (typeof DEFAULT_SETTINGS[key] === "boolean" && typeof value === "boolean") {
      (settings as unknown as Record<string, boolean>)[key] = value;
    }
  }
  for (const key of SITE_LISTS) {
    const value = values[key];
    if (Array.isArray(value)) settings[key] = value.filter((site): site is string => typeof site === "string");
  }
  return settings;
}

/** What to do when a call starts on a site. */
export type SiteChoice = "always" | "never" | "ask";

/**
 * The owner's choice for a site.
 *
 * Inputs: The settings and the site's key (`siteKey`).
 * Outputs: `always` (record without asking), `never` (stay quiet) or `ask`. Side effects: None.
 */
export function choiceForSite(settings: ExtensionSettings, site: string): SiteChoice {
  if (site === "teams" && settings.alwaysRecordTeams) return "always";
  if (settings.alwaysRecordSites.includes(site)) return "always";
  if (settings.neverAskSites.includes(site)) return "never";
  return "ask";
}

/**
 * The settings to change to record a choice for one site.
 *
 * Inputs: The current settings, the site's key and the choice (`ask` forgets the site).
 * Outputs: The changed settings only, for `saveSettings`; for Teams, "always" sets
 * `alwaysRecordTeams`. Side effects: None.
 */
export function withSiteChoice(
  settings: ExtensionSettings,
  site: string,
  choice: SiteChoice,
): Partial<ExtensionSettings> {
  if (site === "teams" && choice === "always") return { alwaysRecordTeams: true };
  const without = (list: readonly string[]) => list.filter((entry) => entry !== site);
  return {
    alwaysRecordSites: choice === "always" ? [...without(settings.alwaysRecordSites), site] : without(settings.alwaysRecordSites),
    neverAskSites: choice === "never" ? [...without(settings.neverAskSites), site] : without(settings.neverAskSites),
  };
}
