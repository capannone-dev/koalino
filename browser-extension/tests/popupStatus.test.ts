/**
 * Specifies the popup's connection line (#212): checked on opening, so an unknown state reads
 * "Checking…" rather than asking the owner to press a button.
 */
import { describe, expect, it } from "vitest";
import { connectionStatus } from "../src/lib/popupStatus";

describe("connectionStatus", () => {
  it("names each state with its tone", () => {
    expect(connectionStatus({ host: "missing", bridge: "unknown" })).toEqual({ key: "helperMissing", tone: "bad" });
    expect(connectionStatus({ host: "connected", bridge: "connected" })).toEqual({ key: "connectedToApp", tone: "ok" });
    expect(connectionStatus({ host: "connected", bridge: "unavailable" })).toEqual({ key: "appNotRunning", tone: "warn" });
    expect(connectionStatus({ host: "unknown", bridge: "unknown" })).toEqual({ key: "checking", tone: "checking" });
  });
});
