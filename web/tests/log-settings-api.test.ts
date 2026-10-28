// Verifies the API client's log settings calls (Issue #46): reading and changing the daemon's log
// level and downloading its logs, with each failure mapped to a kind the page can word.

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { fetchLogExport, getLogLevel, LogSettingsError, setLogLevel } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

test("getLogLevel reads the level in effect", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ level: "info" }), { status: 200 }));

  await expect(getLogLevel()).resolves.toBe("info");
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/settings/log-level", { cache: "no-store" });
});

test("setLogLevel puts the level and returns the one applied", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ level: "debug" }), { status: 200 }));

  await expect(setLogLevel("debug")).resolves.toBe("debug");
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/settings/log-level", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ level: "debug" })
  });
});

test.each([
  [400, { error_code: "invalid_level" }, "invalid_level"],
  [503, { error_code: "unavailable" }, "unavailable"],
  [500, { error_code: "logging_unavailable" }, "server"]
])("setLogLevel maps %i to %s", async (status, body, kind) => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify(body), { status }));

  await expect(setLogLevel("debug")).rejects.toMatchObject({ kind });
});

test("an unreachable daemon is a network failure", async () => {
  fetchMock.mockRejectedValue(new TypeError("fetch failed"));

  await expect(getLogLevel()).rejects.toBeInstanceOf(LogSettingsError);
  await expect(getLogLevel()).rejects.toMatchObject({ kind: "network" });
});

test("fetchLogExport returns the file and its name from the attachment header", async () => {
  fetchMock.mockResolvedValue(
    new Response('{"level":"INFO"}\n', {
      status: 200,
      headers: { "content-disposition": 'attachment; filename="note_taker_logs_20261028-090507.jsonl"' }
    })
  );

  const download = await fetchLogExport();

  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/logs/export", { cache: "no-store" });
  expect(download.fileName).toBe("note_taker_logs_20261028-090507.jsonl");
  expect(download.blob.size).toBe(17);
});

test("fetchLogExport reports no logs yet", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ error_code: "no_logs" }), { status: 404 }));

  await expect(fetchLogExport()).rejects.toMatchObject({ kind: "no_logs" });
});
