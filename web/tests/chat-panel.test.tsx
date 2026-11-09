// Verifies the web chat (#97): the kept chat is shown, a question is sent with its topic and the
// answer says what it was given, a failure is worded, and the chat is deleted.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { ChatPanel } from "@/components/chat-panel";
import * as api from "@/lib/chat-api";

vi.mock("@/lib/chat-api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/chat-api")),
  getChat: vi.fn(),
  askChat: vi.fn(),
  clearChat: vi.fn()
}));

function message(role: "user" | "assistant", text: string, included: string[] = []): api.ChatMessage {
  return {
    role,
    text,
    created_at: "2026-11-09T12:00:00Z",
    included,
    omitted: [],
    connection: role === "assistant" ? "Office" : null
  };
}

beforeEach(() => {
  vi.mocked(api.getChat).mockReset().mockResolvedValue([message("user", "Who came?"), message("assistant", "Anna and Bo.")]);
  vi.mocked(api.askChat).mockReset();
  vi.mocked(api.clearChat).mockReset();
});

test("a question is sent and the answer says what it was given", async () => {
  vi.mocked(api.askChat).mockResolvedValue({
    question: message("user", "When do we ship?"),
    answer: message("assistant", "On Friday.", ['Meeting "Review"'])
  });
  render(<ChatPanel topic={{ scope: "meeting", id: "m-1" }} heading="Chat about this meeting" />);
  expect(await screen.findByText("Anna and Bo.")).toBeInTheDocument();
  expect(api.getChat).toHaveBeenCalledWith({ scope: "meeting", id: "m-1" });

  fireEvent.change(screen.getByLabelText("Your question"), { target: { value: "When do we ship?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask" }));

  expect(await screen.findByText("On Friday.")).toBeInTheDocument();
  expect(screen.getByText('Given: Meeting "Review"')).toBeInTheDocument();
  expect(api.askChat).toHaveBeenCalledWith({ scope: "meeting", id: "m-1" }, "When do we ship?");
  expect(screen.getByLabelText("Your question")).toHaveValue("");
});

test("a failure is worded and the question stays", async () => {
  vi.mocked(api.askChat).mockRejectedValue(new api.ChatApiError("no_connection", 409));
  render(<ChatPanel topic={{ scope: "all" }} heading="Chat with all your notes" />);
  await screen.findByText("Who came?");

  fireEvent.change(screen.getByLabelText("Your question"), { target: { value: "Anything?" } });
  fireEvent.click(screen.getByRole("button", { name: "Ask" }));

  expect(await screen.findByText(/Add an AI connection/)).toBeInTheDocument();
  expect(screen.getByLabelText("Your question")).toHaveValue("Anything?");
});

test("the chat is deleted after confirming", async () => {
  vi.spyOn(window, "confirm").mockReturnValue(true);
  vi.mocked(api.clearChat).mockResolvedValue(undefined);
  render(<ChatPanel topic={{ scope: "project", id: "p-1" }} heading="Chat about this project" />);
  await screen.findByText("Who came?");

  fireEvent.click(screen.getByRole("button", { name: "Delete chat" }));

  expect(await screen.findByText("Chat deleted.")).toBeInTheDocument();
  expect(screen.queryByText("Who came?")).not.toBeInTheDocument();
  expect(api.clearChat).toHaveBeenCalledWith({ scope: "project", id: "p-1" });
});
