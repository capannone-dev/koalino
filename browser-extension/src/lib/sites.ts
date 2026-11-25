/**
 * The meeting sites the extension knows (#174), in one place: `build.mjs` takes their match
 * patterns for the manifest, the page hook and content script tell from them which service a page
 * is, and per-site choices ("Always on this site", "Never on this site") are keyed by them.
 *
 * On a known site the extension watches for calls by default. Any other site is recorded when the
 * owner presses "Record this tab", or watched once the owner allows every site. Teams keeps its
 * own adapter (captions, slides, the Teams transcript); elsewhere the extension records the sound
 * and the shared screen only.
 */

/** A meeting service. */
export interface MeetingService {
  /** A stable key for per-site choices. */
  id: "teams" | "meet" | "zoom" | "jitsi" | "webex" | "whereby";
  /** The name shown. */
  name: string;
  /** Its hosts: an exact name, or `*.` followed by a domain for its subdomains. */
  hosts: readonly string[];
  /** Words its pages put in their title besides the meeting's name, dropped from titles. */
  titleWords: readonly string[];
  /** Match patterns of its pages, for the manifest. */
  patterns: readonly string[];
}

/** Builds a service, deriving its match patterns from its hosts. */
function service(
  id: MeetingService["id"],
  name: string,
  hosts: readonly string[],
  titleWords: readonly string[],
): MeetingService {
  return { id, name, hosts, titleWords, patterns: hosts.map((host) => `https://${host}/*`) };
}

/** The meeting services, Teams first. */
export const MEETING_SERVICES: readonly MeetingService[] = [
  service("teams", "Microsoft Teams", ["teams.microsoft.com", "teams.cloud.microsoft", "teams.live.com"], [
    "Microsoft Teams",
    "Teams",
  ]),
  service("meet", "Google Meet", ["meet.google.com"], ["Google Meet", "Meet"]),
  service("zoom", "Zoom", ["*.zoom.us"], ["Zoom Workplace", "Zoom Meeting", "Zoom"]),
  service("jitsi", "Jitsi Meet", ["meet.jit.si", "8x8.vc"], ["Jitsi Meet", "Jitsi", "8x8"]),
  service("webex", "Webex", ["*.webex.com"], ["Cisco Webex", "Webex Meetings", "Webex"]),
  service("whereby", "Whereby", ["whereby.com"], ["Whereby"]),
];

/** Whether `hostname` is `host` (an exact name, or `*.domain` for any subdomain of it). */
function hostMatches(hostname: string, host: string): boolean {
  if (!host.startsWith("*.")) return hostname === host;
  return hostname.endsWith(host.slice(1));
}

/**
 * The service a page belongs to.
 *
 * Inputs: The page's host name. Outputs: The service, or null for any other site.
 * Side effects: None.
 */
export function serviceForHost(hostname: string): MeetingService | null {
  const host = hostname.toLowerCase();
  return MEETING_SERVICES.find((candidate) => candidate.hosts.some((pattern) => hostMatches(host, pattern))) ?? null;
}

/**
 * Every known service's match patterns, for the manifest.
 *
 * Inputs: None. Outputs: The patterns, each once. Side effects: None.
 */
export function knownSitePatterns(): string[] {
  return [...new Set(MEETING_SERVICES.flatMap((candidate) => candidate.patterns))];
}

/** A host without a leading `www.`, lower case. */
function bareHost(hostname: string): string {
  return hostname.toLowerCase().replace(/^www\./, "");
}

/**
 * The name shown for a site: its service's, else its host.
 *
 * Inputs: The host name. Outputs: The name. Side effects: None.
 */
export function siteName(hostname: string): string {
  return serviceForHost(hostname)?.name ?? bareHost(hostname);
}

/**
 * The key of a site's choices: its service's ID (one for all of Zoom's hosts), else its host.
 *
 * Inputs: The host name. Outputs: The key. Side effects: None.
 */
export function siteKey(hostname: string): string {
  return serviceForHost(hostname)?.id ?? bareHost(hostname);
}

/** Separators between the parts of a page title. */
const TITLE_SEPARATOR = /\s+[-–—|·:]\s+|^\s*[-–—|·:]\s*|\s*[-–—|·:]\s*$/;
/** A Google Meet code ("abc-defg-hij"). */
const MEET_CODE = /^[a-z]{3}-[a-z]{4}-[a-z]{3}$/;

/**
 * The meeting's title from a page's title.
 *
 * Inputs: The page's host name and `document.title`.
 * Outputs: The title without the service's own words and the unread count some pages put first
 * ("(2) "); a Google Meet code as "Google Meet <code>"; "" when nothing else is left.
 * Side effects: None.
 */
export function meetingTitle(hostname: string, documentTitle: string): string {
  const known = serviceForHost(hostname);
  const cleaned = documentTitle.replace(/^\s*\(\d+\)\s*/, "").trim();
  if (!known) return cleaned;
  const ownWords = new Set(known.titleWords.map((word) => word.toLowerCase()));
  const parts = cleaned
    .split(TITLE_SEPARATOR)
    .map((part) => part.trim())
    .filter((part) => part !== "" && !ownWords.has(part.toLowerCase()));
  const title = parts.join(" – ");
  if (known.id === "meet" && MEET_CODE.test(title)) return `${known.name} ${title}`;
  return title;
}
