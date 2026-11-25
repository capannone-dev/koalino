/**
 * Brings the extension's scripts to sites beyond the known meeting sites (#174).
 *
 * - "Watch every website for calls": once the owner allows all sites, the background registers
 *   the page hook and the content script for every https page except the sites the manifest
 *   already covers, and removes them when the owner turns it off or withdraws the access.
 *   `syncAnySiteScripts` runs at every background start, so the registration follows the choice
 *   even where the browser does not keep it across restarts.
 * - "Record this tab": the owner opened the popup on the tab, so `activeTab` lets the background
 *   inject both scripts there; `ensureTabScripts` injects them only where the content script does
 *   not run yet. The hook then arrives mid-call; it still finds the call connections, because the
 *   page keeps calling them (`observeConnections`).
 *
 * The browser API is passed in (`ScriptingApi`), so this runs in tests.
 */
import { CONTENT_LOADED_KEY } from "./messages";

/** Every https site, as allowed from the popup. */
export const ANY_SITE_PATTERNS = ["https://*/*"];
/** SharePoint pages: the manifest's recap-frame scripts are the only ones there. */
const SHAREPOINT_PATTERNS = ["https://*.sharepoint.com/*"];
/** The IDs of the registered scripts. */
export const ANY_SITE_SCRIPT_IDS = ["koalino-any-site-hook", "koalino-any-site-content"] as const;

/** A content script to register (the part of `chrome.scripting.RegisteredContentScript` used). */
export interface ScriptRegistration {
  id: string;
  matches: string[];
  excludeMatches: string[];
  js: string[];
  runAt: "document_start" | "document_idle";
  world?: "MAIN" | "ISOLATED";
  allFrames: boolean;
}

/** A script to run in a tab now (the part of `chrome.scripting.ScriptInjection` used). */
export interface ScriptInjection {
  target: { tabId: number };
  files?: string[];
  func?: (key: string) => boolean;
  args?: string[];
  world?: "MAIN" | "ISOLATED";
}

/** The part of `chrome.scripting` used here. */
export interface ScriptingApi {
  getRegisteredContentScripts(filter?: { ids?: string[] }): Promise<{ id: string }[]>;
  registerContentScripts(scripts: ScriptRegistration[]): Promise<void>;
  unregisterContentScripts(filter?: { ids?: string[] }): Promise<void>;
  executeScript(injection: ScriptInjection): Promise<{ result?: unknown }[]>;
}

/**
 * The scripts registered for every site.
 *
 * Inputs: The known meeting sites' patterns (the manifest already runs the scripts there).
 * Outputs: The page hook (page world, document start) and the content script (document idle),
 * top frames only, on every https page but the known sites and SharePoint. Side effects: None.
 */
export function anySiteScripts(knownSites: readonly string[]): ScriptRegistration[] {
  const excludeMatches = [...knownSites, ...SHAREPOINT_PATTERNS];
  const [hookId, contentId] = ANY_SITE_SCRIPT_IDS;
  return [
    {
      id: hookId,
      matches: ANY_SITE_PATTERNS,
      excludeMatches,
      js: ["hook.js"],
      runAt: "document_start",
      world: "MAIN",
      allFrames: false,
    },
    { id: contentId, matches: ANY_SITE_PATTERNS, excludeMatches, js: ["content.js"], runAt: "document_idle", allFrames: false },
  ];
}

/**
 * Registers or removes the every-site scripts to match the owner's choice.
 *
 * Inputs: The scripting API; whether the owner wants every site watched, whether the browser
 * grants access to all sites, and the known sites' patterns.
 * Outputs: None. Side effects: Registers both scripts when wanted, allowed and missing; removes
 * them when registered but not wanted or not allowed.
 */
export async function syncAnySiteScripts(
  scripting: ScriptingApi,
  choice: { wanted: boolean; allowed: boolean; knownSites: readonly string[] },
): Promise<void> {
  const ids = [...ANY_SITE_SCRIPT_IDS];
  const registered = await scripting.getRegisteredContentScripts({ ids });
  const present = ids.every((id) => registered.some((script) => script.id === id));
  if (choice.wanted && choice.allowed) {
    if (present) return;
    if (registered.length > 0) await scripting.unregisterContentScripts({ ids });
    await scripting.registerContentScripts(anySiteScripts(choice.knownSites));
    return;
  }
  if (registered.length > 0) await scripting.unregisterContentScripts({ ids });
}

/** Whether the content script set its flag in this world (runs in the tab). */
function contentLoaded(key: string): boolean {
  return (globalThis as Record<string, unknown>)[key] === true;
}

/**
 * Brings the page hook and the content script into a tab, for "Record this tab".
 *
 * Inputs: The scripting API and the tab's ID. Outputs: None; rejects when the browser refuses
 * the tab (a browser page, or no access to it).
 * Side effects: Injects the hook into the page's world and then the content script, unless the
 * content script already runs there.
 */
export async function ensureTabScripts(scripting: ScriptingApi, tabId: number): Promise<void> {
  const [probe] = await scripting.executeScript({ target: { tabId }, func: contentLoaded, args: [CONTENT_LOADED_KEY] });
  if (probe?.result === true) return;
  await scripting.executeScript({ target: { tabId }, files: ["hook.js"], world: "MAIN" });
  await scripting.executeScript({ target: { tabId }, files: ["content.js"] });
}
