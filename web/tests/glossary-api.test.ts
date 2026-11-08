// Verifies the API client's glossary calls: reading and replacing the terms, with each failure
// mapped to a kind the page can word.

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { getGlossary, GlossaryError, setGlossary } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

test("getGlossary reads the saved terms", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ terms: ["OKR"] }), { status: 200 }));

  await expect(getGlossary()).resolves.toEqual(["OKR"]);
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/settings/glossary", { cache: "no-store" });
});

test("setGlossary puts the terms and returns them as saved", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ terms: ["Marco Rossi"] }), { status: 200 }));

  await expect(setGlossary([" Marco Rossi "])).resolves.toEqual(["Marco Rossi"]);
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/settings/glossary", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ terms: [" Marco Rossi "] })
  });
});

test.each([
  [400, { error_code: "invalid_glossary" }, "invalid_glossary"],
  [503, { error_code: "unavailable" }, "unavailable"],
  [500, { error_code: "save_failed" }, "server"]
])("setGlossary maps %i to %s", async (status, body, kind) => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify(body), { status }));

  await expect(setGlossary(["x"])).rejects.toEqual(new GlossaryError(kind as never, status));
});

test("an unreachable server is a network failure", async () => {
  fetchMock.mockRejectedValue(new TypeError("fetch failed"));

  await expect(getGlossary()).rejects.toEqual(new GlossaryError("network", null));
});
