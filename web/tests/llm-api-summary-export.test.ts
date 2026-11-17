// Verifies downloading a meeting's summary as a file (#130): the request, the file name from
// `Content-Disposition` (UTF-8 names included), and failures.

import { afterEach, expect, test, vi } from "vitest";
import { LlmApiError, fetchSummaryExport } from "@/lib/llm-api";

afterEach(() => vi.unstubAllGlobals());

test("downloads the summary with the name the server gave", async () => {
  const fetchMock = vi.fn().mockResolvedValue(
    new Response("%PDF", {
      status: 200,
      headers: {
        "content-disposition": "attachment; filename=\"Caf_ - summary.pdf\"; filename*=UTF-8''Caf%C3%A9%20-%20summary.pdf"
      }
    })
  );
  vi.stubGlobal("fetch", fetchMock);

  const download = await fetchSummaryExport("m 1", "pdf");

  expect(download.fileName).toBe("Café - summary.pdf");
  expect(download.blob.size).toBe(4);
  expect(String(fetchMock.mock.calls[0][0])).toMatch(/\/api\/meetings\/m%201\/summary\/export\?format=pdf$/);
});

test("a refusal becomes an LlmApiError with the server's code", async () => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(Response.json({ ok: false, error_code: "no_summary" }, { status: 409 })));
  await expect(fetchSummaryExport("m-1", "md")).rejects.toEqual(new LlmApiError("no_summary", 409));
});
