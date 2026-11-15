// Typed client for the daemon's notes folder (#98): plain files mirroring projects and meetings
// on the daemon's computer, turned on and off from Settings.

import { apiBaseUrl } from "@/lib/api";
import { englishT, type Translate } from "@/lib/i18n";

/** The folder's settings and how its last round went, as the daemon describes them. */
export type NotesFolderStatus = {
  enabled: boolean;
  path: string | null;
  suggested_path: string | null;
  last_round_at: string | null;
  last_error: string | null;
  written: number;
  imported: number;
};

/** A failed request: the daemon's error code (or `network`) and the HTTP status. */
export class NotesFolderError extends Error {
  constructor(
    readonly code: string,
    readonly status: number | null
  ) {
    super(`Notes folder request failed (${status ?? "network"}): ${code}`);
    this.name = "NotesFolderError";
  }
}

/** Plain messages per error code; server text is not shown. */
const messages = (t: Translate) : Record<string, string> => ({
  network: t("cantReachTheNoteTaking"),
  invalid_folder: t("chooseAFullFolderPath"),
  folder_failed: t("theFolderCouldNotBe"),
  storage_failed: t("theNotesCouldNotBe2"),
  unavailable: t("thisServerKeepsNoNotes")
});

/** Words an error code for people. */
export function notesFolderMessage(code: string, t: Translate = englishT): string {
  return messages(t)[code] ?? t("somethingWentWrongOnThe");
}

/** Sends one request, mapping failures to `NotesFolderError`. */
async function request(path: string, init: RequestInit): Promise<NotesFolderStatus> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, init);
  } catch {
    throw new NotesFolderError("network", null);
  }
  const body = (await response.json().catch(() => ({}))) as { error_code?: unknown };
  if (!response.ok) {
    throw new NotesFolderError(typeof body.error_code === "string" ? body.error_code : "server", response.status);
  }
  return body as NotesFolderStatus;
}

/**
 * Reads the status (`GET /api/settings/notes-folder`). Throws `NotesFolderError`. Side effects: none.
 */
export async function getNotesFolder(): Promise<NotesFolderStatus> {
  return request("/api/settings/notes-folder", { cache: "no-store" });
}

/**
 * Turns the folder on or off (`PUT /api/settings/notes-folder`).
 *
 * Inputs: whether to keep it, and the folder (absolute, on the daemon's computer); without one the
 * saved or suggested folder is used. Throws `NotesFolderError`. Side effects: saves the setting;
 * turning it on writes the folder at once.
 */
export async function setNotesFolder(enabled: boolean, path: string | null): Promise<NotesFolderStatus> {
  return request("/api/settings/notes-folder", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(path ? { enabled, path } : { enabled })
  });
}

/** Reads edits back and writes the folder now (`POST /api/settings/notes-folder/sync`). */
export async function syncNotesFolder(): Promise<NotesFolderStatus> {
  return request("/api/settings/notes-folder/sync", { method: "POST" });
}
