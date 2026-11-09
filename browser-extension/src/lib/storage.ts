/**
 * Reads and writes the extension's state in `chrome.storage.local`: the settings, and the record
 * of meetings sent to Note Taker (titles and times only, kept two weeks).
 */
import { RecordedMeeting, pruneMeetings } from "./meetings";
import { ExtensionSettings, normalizeSettings } from "./settings";

/** Storage keys. */
const SETTINGS_KEY = "settings";
const MEETINGS_KEY = "meetings";

/** Reads the settings. Side effects: Reads storage. */
export async function loadSettings(): Promise<ExtensionSettings> {
  const stored = await chrome.storage.local.get(SETTINGS_KEY);
  return normalizeSettings(stored[SETTINGS_KEY]);
}

/** Changes some settings. Side effects: Writes storage. */
export async function saveSettings(changes: Partial<ExtensionSettings>): Promise<ExtensionSettings> {
  const settings = { ...(await loadSettings()), ...changes };
  await chrome.storage.local.set({ [SETTINGS_KEY]: settings });
  return settings;
}

/** Whether a storage change touched the settings. */
export function settingsChanged(changes: Record<string, chrome.storage.StorageChange>): boolean {
  return SETTINGS_KEY in changes;
}

/** Reads the meeting records. Side effects: Reads storage. */
export async function loadMeetings(): Promise<RecordedMeeting[]> {
  const stored = await chrome.storage.local.get(MEETINGS_KEY);
  const meetings = stored[MEETINGS_KEY];
  return Array.isArray(meetings) ? (meetings as RecordedMeeting[]) : [];
}

/**
 * Adds or replaces one meeting record, dropping records older than two weeks.
 * Side effects: Reads and writes storage.
 */
export async function saveMeeting(meeting: RecordedMeeting): Promise<void> {
  const others = (await loadMeetings()).filter((existing) => existing.meetingId !== meeting.meetingId);
  await chrome.storage.local.set({ [MEETINGS_KEY]: pruneMeetings([...others, meeting], Date.now()) });
}
