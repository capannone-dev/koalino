/**
 * The extension's settings (kept in `chrome.storage.local`, per browser profile), with their
 * defaults. Every capture is off until the owner agrees to record a meeting; starting Teams
 * transcription is a separate opt-in because it is visible to everyone and stores the transcript
 * in Microsoft's cloud.
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
}

/** The settings before the owner changes anything. */
export const DEFAULT_SETTINGS: ExtensionSettings = {
  alwaysRecordTeams: false,
  startTeamsTranscription: false,
  captureSlides: true,
  captureCaptions: true,
  fetchTeamsTranscripts: true,
};

/**
 * Reads stored settings, keeping only known boolean values.
 *
 * Inputs: Whatever storage returned. Outputs: Complete settings. Side effects: None.
 */
export function normalizeSettings(stored: unknown): ExtensionSettings {
  const settings = { ...DEFAULT_SETTINGS };
  if (typeof stored !== "object" || stored === null) return settings;
  for (const key of Object.keys(DEFAULT_SETTINGS) as (keyof ExtensionSettings)[]) {
    const value = (stored as Record<string, unknown>)[key];
    if (typeof value === "boolean") settings[key] = value;
  }
  return settings;
}
