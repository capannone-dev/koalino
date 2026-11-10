/**
 * Checks and asks for the extension's access to the Teams sites (Issue #102).
 *
 * Firefox does not grant a Manifest V3 extension its host permissions when it is loaded by hand
 * (about:debugging or a file), and Chrome lets the owner withhold them. Without them the content
 * scripts never run on Teams, so nothing is recorded and nothing says why. The sites come from the
 * manifest's `host_permissions`, so the list lives only in `build.mjs`. They include SharePoint,
 * where Teams shows a meeting recap's transcript in a frame; there the extension acts only in
 * frames a Teams page opened (`embeddedInTeams`).
 */

/** A Teams page's address: the sites in `build.mjs`. */
const TEAMS_PAGE = /^https:\/\/teams\.(microsoft\.com|cloud\.microsoft|live\.com)\//;

/**
 * Whether this frame is one a Teams page opened (the recap transcript frame on SharePoint).
 *
 * Inputs: Whether this is the top frame, and `document.referrer` (the embedding page's address).
 * Outputs: `true` for a sub-frame opened by a Teams page. Side effects: None.
 */
export function embeddedInTeams(isTopFrame: boolean, referrer: string): boolean {
  return !isTopFrame && TEAMS_PAGE.test(referrer);
}

/** The part of `chrome.permissions` used here, so specs can pass a fake. */
export interface PermissionsApi {
  contains(permissions: { origins?: string[] }): Promise<boolean>;
  request(permissions: { origins?: string[] }): Promise<boolean>;
}

/** The part of the manifest used here (a Manifest V2 one has no `host_permissions`). */
export type ManifestWithHosts = object & { host_permissions?: string[] };

/**
 * The Teams site patterns the extension needs.
 *
 * Inputs: The extension's manifest (`chrome.runtime.getManifest()`).
 * Outputs: Its `host_permissions`, or an empty list. Side effects: None.
 */
export function teamsOrigins(manifest: ManifestWithHosts): string[] {
  return "host_permissions" in manifest && Array.isArray(manifest.host_permissions)
    ? [...manifest.host_permissions]
    : [];
}

/**
 * Whether the extension may run on every Teams site.
 *
 * Inputs: The permissions API and the manifest.
 * Outputs: `true` when all are granted. Side effects: None.
 */
export async function hasTeamsAccess(permissions: PermissionsApi, manifest: ManifestWithHosts): Promise<boolean> {
  return permissions.contains({ origins: teamsOrigins(manifest) });
}

/**
 * Asks the owner for access to the Teams sites. It must run in a user action (a click).
 *
 * Inputs: The permissions API and the manifest.
 * Outputs: `true` when the owner allowed it.
 * Side effects: The browser shows its permission prompt.
 */
export async function requestTeamsAccess(
  permissions: PermissionsApi,
  manifest: ManifestWithHosts,
): Promise<boolean> {
  return permissions.request({ origins: teamsOrigins(manifest) });
}
