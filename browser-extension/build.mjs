/**
 * Builds the extension for Firefox and for Chromium browsers (Chrome, Edge, Chromium) into
 * dist/firefox and dist/chromium: bundles the TypeScript entry points with esbuild, copies the
 * static files, and writes each browser's manifest.
 *
 * Usage: node build.mjs
 */
import { build } from "esbuild";
import { cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";

/** The Teams sites the extension works on. */
const TEAMS_SITES = [
  "https://teams.microsoft.com/*",
  "https://teams.cloud.microsoft/*",
  "https://teams.live.com/*",
];

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
  name: "Koalino for Teams",
  version: "0.1.0",
  description: "Records Teams meetings in the browser and sends them to the Koalino app on this computer.",
  permissions: ["nativeMessaging", "storage"],
  host_permissions: [...TEAMS_SITES, ...RECAP_FRAME_SITES],
  icons: { 48: "icon-48.png", 128: "icon-128.png" },
  action: { default_popup: "popup.html", default_icon: { 48: "icon-48.png" } },
  content_scripts: [
    { matches: TEAMS_SITES, js: ["hook.js"], run_at: "document_start", world: "MAIN" },
    { matches: TEAMS_SITES, js: ["content.js"], run_at: "document_idle" },
    { matches: RECAP_FRAME_SITES, js: ["hook.js"], run_at: "document_start", world: "MAIN", all_frames: true },
    { matches: RECAP_FRAME_SITES, js: ["recap-frame.js"], run_at: "document_idle", all_frames: true },
  ],
};

/** Each browser's manifest. */
const manifests = {
  firefox: {
    ...baseManifest,
    background: { scripts: ["background.js"] },
    browser_specific_settings: { gecko: { id: FIREFOX_ADDON_ID, strict_min_version: "128.0" } },
  },
  chromium: {
    ...baseManifest,
    key: CHROMIUM_PUBLIC_KEY,
    minimum_chrome_version: "111",
    background: { service_worker: "background.js" },
  },
};

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
      "recap-frame": "src/recap/recapFrame.ts",
    },
    bundle: true,
    format: "iife",
    target: browser === "firefox" ? "firefox128" : "chrome111",
    outdir,
    logLevel: "warning",
  });
  cpSync("static", outdir, { recursive: true });
  writeFileSync(`${outdir}/manifest.json`, JSON.stringify(manifest, null, 2));
  console.log(`built ${outdir}`);
}
