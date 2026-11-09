/**
 * Specifies how the extension checks and asks for access to the Teams sites. Firefox does not grant
 * a Manifest V3 extension its host permissions when it is loaded by hand, and Chrome lets the owner
 * withhold them. Without that access the content scripts never run on Teams.
 */
import { describe, expect, it } from "vitest";
import { PermissionsApi, hasTeamsAccess, requestTeamsAccess, teamsOrigins } from "../src/lib/siteAccess";

const MANIFEST = {
  host_permissions: ["https://teams.microsoft.com/*", "https://teams.cloud.microsoft/*", "https://teams.live.com/*"],
};

/** A permissions API holding `granted` and answering requests with `answer`. */
function fakePermissions(granted: string[], answer = true): PermissionsApi & { requested: string[][] } {
  const requested: string[][] = [];
  return {
    requested,
    contains: async ({ origins }) => (origins ?? []).every((origin) => granted.includes(origin)),
    request: async ({ origins }) => {
      requested.push(origins ?? []);
      if (answer) granted.push(...(origins ?? []));
      return answer;
    },
  };
}

describe("teamsOrigins", () => {
  it("reads the Teams sites from the manifest", () => {
    expect(teamsOrigins(MANIFEST)).toEqual(MANIFEST.host_permissions);
  });

  it("is empty for a manifest without host permissions", () => {
    expect(teamsOrigins({})).toEqual([]);
  });
});

describe("hasTeamsAccess", () => {
  it("is true when every Teams site is granted", async () => {
    expect(await hasTeamsAccess(fakePermissions([...MANIFEST.host_permissions]), MANIFEST)).toBe(true);
  });

  it("is false when one Teams site is missing", async () => {
    const granted = MANIFEST.host_permissions.slice(1);
    expect(await hasTeamsAccess(fakePermissions(granted), MANIFEST)).toBe(false);
  });
});

describe("requestTeamsAccess", () => {
  it("asks for every Teams site and reports the owner's answer", async () => {
    const permissions = fakePermissions([]);
    expect(await requestTeamsAccess(permissions, MANIFEST)).toBe(true);
    expect(permissions.requested).toEqual([MANIFEST.host_permissions]);
    expect(await hasTeamsAccess(permissions, MANIFEST)).toBe(true);
  });

  it("reports a refusal", async () => {
    expect(await requestTeamsAccess(fakePermissions([], false), MANIFEST)).toBe(false);
  });
});
