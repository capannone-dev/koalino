/**
 * Builds the extension for Firefox and for Chromium browsers (Chrome, Edge, Chromium) into
 * dist/firefox and dist/chromium: bundles the TypeScript entry points with esbuild, copies the
 * static files, writes each language's `_locales/<lang>/messages.json` from `src/locales` (#124),
 * and writes each browser's manifest.
 *
 * Usage: node build.mjs
 */
import { build } from "esbuild";
import { cpSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { chromeLocaleDirectory, chromeMessages } from "./src/lib/localeFormat.ts";
import { knownSitePatterns } from "./src/lib/sites.ts";

/**
 * The meeting sites the extension watches for calls without being asked (#174): Teams, Google
 * Meet, Zoom, Jitsi, Webex and Whereby, listed in `src/lib/sites.ts`.
 */
const MEETING_SITES = knownSitePatterns();

/**
 * Every other site, which the owner may allow from the popup ("Watch every website for calls");
 * the background then registers the scripts there (#174). "Record this tab" needs no such access:
 * `activeTab` grants the one tab when the owner opens the popup.
 */
const ANY_SITE = ["https://*/*"];

/**
 * SharePoint, where Teams shows a meeting recap's transcript in a frame (Stream's player). The
 * scripts there act only in frames a Teams page opened (`embeddedInTeams`).
 */
const RECAP_FRAME_SITES = ["https://*.sharepoint.com/*"];

/**
 * The public key that fixes the Chromium extension ID (dgmaacilnokmfecfdcajdjadeboemnpd), which
 * the native host's manifest allows. Only the public half: the private key is not needed to load
 * the extension unpacked.
 */
const CHROMIUM_PUBLIC_KEY = "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAvJkM1kRGdBQziDy+BCt3HKNtGA8VVwvcx+r5AN7TVOczKc28cUawmBDsAJs8Zddgwy06VIyU8Ka7mWwkl0WdffhTs2SRhs7zRP2LFcQkHoEBJ1JMGo02g7cnkX++8fZwt9hh7wzujPhuusi9nyakRPD1FJr5FVxJYuwvxhbp6FidSJNnEZbl3pC6hapU8132Hsx9cVv77ulI74eyQ6eZx/eQFsNSgtnRXCKMeyl+fkYolXwK6NarmfVfax9Pf2u0OBcsZu4ky3MdYG/wtxSDEqARLqYYxUPpvaUQB3+TY4wP+uyQLbkIci5ah31kvQsftF5Us2yjdynBh2ztWXeQ9QIDAQAB";

/** The Firefox add-on ID the native host's manifest allows. */
const FIREFOX_ADDON_ID = "browser-bridge@note-taker.local";

/** The manifest parts both browsers share. */
const baseManifest = {
  manifest_version: 3,
  name: "__MSG_extName__",
  version: "0.2.1",
  description: "__MSG_extDescription__",
  default_locale: "en",
  permissions: ["nativeMessaging", "storage", "scripting", "activeTab"],
  host_permissions: [...MEETING_SITES, ...RECAP_FRAME_SITES],
  optional_host_permissions: ANY_SITE,
  icons: { 16: "icon-16.png", 32: "icon-32.png", 48: "icon-48.png", 128: "icon-128.png" },
  action: { default_popup: "popup.html", default_icon: { 16: "icon-16.png", 32: "icon-32.png", 48: "icon-48.png" } },
  // The settings, grouped, in a page of their own (#212); the popup links to it.
  options_ui: { page: "options.html", open_in_tab: true },
  content_scripts: [
    { matches: MEETING_SITES, js: ["hook.js"], run_at: "document_start", world: "MAIN" },
    { matches: MEETING_SITES, js: ["content.js"], run_at: "document_idle" },
    { matches: RECAP_FRAME_SITES, js: ["hook.js"], run_at: "document_start", world: "MAIN", all_frames: true },
    { matches: RECAP_FRAME_SITES, js: ["recap-frame.js"], run_at: "document_idle", all_frames: true },
  ],
};

/** Each browser's manifest. */
const manifests = {
  firefox: {
    ...baseManifest,
    background: { scripts: ["background.js"] },
    browser_specific_settings: {
      gecko: {
        id: FIREFOX_ADDON_ID,
        strict_min_version: "140.0",
        // AMO's declaration of data sent to the developer or third parties: none. Meetings go only
        // to the Koalino app on this computer, through the native host (#123).
        data_collection_permissions: { required: ["none"] },
      },
    },
  },
  chromium: {
    ...baseManifest,
    key: CHROMIUM_PUBLIC_KEY,
    minimum_chrome_version: "111",
    background: { service_worker: "background.js" },
  },
};

/**
 * Writes every language's messages into an output folder's `_locales`.
 *
 * Inputs: The output folder. Side effects: Creates `<outdir>/_locales/<lang>/messages.json`.
 */
function writeLocales(outdir) {
  const readLocale = (file) => JSON.parse(readFileSync(`src/locales/${file}`, "utf8"));
  const english = readLocale("en.json");
  for (const file of readdirSync("src/locales").filter((name) => name.endsWith(".json"))) {
    const directory = `${outdir}/_locales/${chromeLocaleDirectory(file.replace(/\.json$/, ""))}`;
    mkdirSync(directory, { recursive: true });
    writeFileSync(`${directory}/messages.json`, JSON.stringify(chromeMessages(readLocale(file), english), null, 2));
  }
}

for (const [browser, manifest] of Object.entries(manifests)) {
  const outdir = `dist/${browser}`;
  rmSync(outdir, { recursive: true, force: true });
  mkdirSync(outdir, { recursive: true });
  await build({
    entryPoints: {
      background: "src/background/background.ts",
      content: "src/content/content.ts",
      hook: "src/page/hook.ts",
      popup: "src/popup/popup.ts",
      options: "src/options/options.ts",
      "recap-frame": "src/recap/recapFrame.ts",
    },
    bundle: true,
    format: "iife",
    target: browser === "firefox" ? "firefox140" : "chrome111",
    outdir,
    logLevel: "warning",
  });
  cpSync("static", outdir, { recursive: true });
  writeLocales(outdir);
  writeFileSync(`${outdir}/manifest.json`, JSON.stringify(manifest, null, 2));
  console.log(`built ${outdir}`);
}
