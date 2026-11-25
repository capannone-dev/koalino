/**
 * Specifies how the background keeps the scripts registered on every website while the owner
 * wants it and allowed it (#174), and how it brings the scripts into a tab for "Record this tab".
 */
import { describe, expect, it } from "vitest";
import {
  ANY_SITE_SCRIPT_IDS,
  ScriptingApi,
  anySiteScripts,
  ensureTabScripts,
  syncAnySiteScripts,
} from "../src/lib/anySite";

/** A fake `chrome.scripting` that records what it was asked. */
function fakeScripting(registered: string[] = [], loaded = false): ScriptingApi & { calls: string[] } {
  const calls: string[] = [];
  return {
    calls,
    getRegisteredContentScripts: async () => registered.map((id) => ({ id })),
    registerContentScripts: async (scripts) => {
      calls.push(`register ${scripts.map((script) => script.id).join(",")}`);
      registered.push(...scripts.map((script) => script.id));
    },
    unregisterContentScripts: async (filter) => {
      calls.push(`unregister ${filter?.ids?.join(",")}`);
      registered.splice(0, registered.length);
    },
    executeScript: async (injection) => {
      if (injection.func) {
        calls.push("probe");
        return [{ result: loaded }];
      }
      calls.push(`inject ${injection.files?.join(",")} ${injection.world ?? "ISOLATED"}`);
      return [];
    },
  };
}

describe("anySiteScripts", () => {
  it("runs the hook early in the page's world and the content script later, outside the known sites", () => {
    const [hook, content] = anySiteScripts(["https://meet.google.com/*"]);
    expect(hook).toMatchObject({ js: ["hook.js"], runAt: "document_start", world: "MAIN", matches: ["https://*/*"] });
    expect(content).toMatchObject({ js: ["content.js"], runAt: "document_idle", matches: ["https://*/*"] });
    expect(hook?.excludeMatches).toContain("https://meet.google.com/*");
    expect(hook?.excludeMatches).toContain("https://*.sharepoint.com/*");
  });
});

describe("syncAnySiteScripts", () => {
  it("registers the scripts when wanted and allowed", async () => {
    const scripting = fakeScripting();
    await syncAnySiteScripts(scripting, { wanted: true, allowed: true, knownSites: [] });
    expect(scripting.calls).toEqual([`register ${ANY_SITE_SCRIPT_IDS.join(",")}`]);
  });

  it("leaves registered scripts alone", async () => {
    const scripting = fakeScripting([...ANY_SITE_SCRIPT_IDS]);
    await syncAnySiteScripts(scripting, { wanted: true, allowed: true, knownSites: [] });
    expect(scripting.calls).toEqual([]);
  });

  it("removes them when no longer wanted or no longer allowed", async () => {
    for (const choice of [
      { wanted: false, allowed: true },
      { wanted: true, allowed: false },
    ]) {
      const scripting = fakeScripting([...ANY_SITE_SCRIPT_IDS]);
      await syncAnySiteScripts(scripting, { ...choice, knownSites: [] });
      expect(scripting.calls).toEqual([`unregister ${ANY_SITE_SCRIPT_IDS.join(",")}`]);
    }
  });

  it("does nothing when nothing is registered and nothing is wanted", async () => {
    const scripting = fakeScripting();
    await syncAnySiteScripts(scripting, { wanted: false, allowed: true, knownSites: [] });
    expect(scripting.calls).toEqual([]);
  });
});

describe("ensureTabScripts", () => {
  it("injects the hook into the page's world, then the content script, where they are missing", async () => {
    const scripting = fakeScripting([], false);
    await ensureTabScripts(scripting, 7);
    expect(scripting.calls).toEqual(["probe", "inject hook.js MAIN", "inject content.js ISOLATED"]);
  });

  it("injects nothing where the content script already runs", async () => {
    const scripting = fakeScripting([], true);
    await ensureTabScripts(scripting, 7);
    expect(scripting.calls).toEqual(["probe"]);
  });
});
