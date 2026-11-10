// Verifies that a rejected AI request (#104) shows the AI server's own reason, read from the
// web API's `error_detail`, for summaries and chats alike.

import { afterEach, expect, test, vi } from "vitest";
import { askChat, chatErrorMessage } from "@/lib/chat-api";
import { llmErrorMessage, summarizeMeeting } from "@/lib/llm-api";

/** Makes every fetch answer a 502 rejection carrying `detail`. */
function rejectWith(detail: string): void {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      new Response(JSON.stringify({ ok: false, error_code: "llm_rejected", error: "rejected", error_detail: detail }), {
        status: 502,
        headers: { "content-type": "application/json" }
      })
    )
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

test("a rejected summary shows the server's reason", async () => {
  rejectWith("invalid_request_error: Your credit balance is too low");
  const error = await summarizeMeeting("m-1").catch((caught: unknown) => caught);
  expect(llmErrorMessage(error)).toBe(
    "The AI server rejected the request. The server said: invalid_request_error: Your credit balance is too low"
  );
});

test("a rejected chat question shows the server's reason", async () => {
  rejectWith("model_not_found: no such model");
  const error = await askChat({ scope: "all" }, "Anything?").catch((caught: unknown) => caught);
  expect(chatErrorMessage(error)).toBe("The AI server rejected the request. The server said: model_not_found: no such model");
});
