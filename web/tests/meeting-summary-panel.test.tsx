// Verifies a meeting's summary panel (#95): showing the stored summary, writing one with a chosen
// connection, the reasons it fails, and the meeting's own prompt.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingSummaryPanel } from "@/components/meeting-summary-panel";
import * as api from "@/lib/llm-api";
import * as edits from "@/lib/edits-api";

vi.mock("@/lib/edits-api", async () => ({
  ...(await vi.importActual<typeof edits>("@/lib/edits-api")),
  editSummaryText: vi.fn()
}));

vi.mock("@/lib/llm-api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/llm-api")),
  summarizeMeeting: vi.fn(),
  setMeetingPrompt: vi.fn()
}));

const connections: api.LlmConnection[] = [
  { id: "a", label: "Office", kind: "open_ai_compatible", base_url: "http://10.0.0.2/v1", model: "llama", context_tokens: 8192, plain_http: true, has_key: false, is_default: false },
  { id: "b", label: "Claude", kind: "anthropic", base_url: "https://api.anthropic.com", model: "claude-opus-5-5", context_tokens: 200000, plain_http: false, has_key: true, is_default: true }
];

const written: api.StoredSummary = {
  text: "## Overview\nBudget approved.",
  connection_label: "Claude",
  model: "claude-opus-5-5",
  created_at: "2026-11-09T10:00:00Z",
  prompt: "p",
  edited: false,
  truncated: false
};

beforeEach(() => {
  vi.mocked(api.summarizeMeeting).mockReset();
  vi.mocked(api.setMeetingPrompt).mockReset();
});

test("writes a summary with the default connection and shows it", async () => {
  vi.mocked(api.summarizeMeeting).mockResolvedValue({ summary: written, parts: 3, images_sent: 0, images_skipped: 1 });
  render(<MeetingSummaryPanel meetingId="m-1" summary={null} meetingPrompt={null} connections={connections} defaultTemplate="T" />);

  expect(screen.getByText("No summary yet.")).toBeInTheDocument();
  expect(screen.getByLabelText("Summarize with")).toHaveValue("b");
  fireEvent.click(screen.getByRole("button", { name: "Summarize" }));

  expect(await screen.findByText(/Summary written\. The transcript was read in 3 parts\. 1 picture/)).toBeInTheDocument();
  expect(screen.getByText(/Budget approved\./)).toBeInTheDocument();
  expect(screen.getByText(/Written by claude-opus-5-5 \(Claude\)/)).toBeInTheDocument();
  expect(api.summarizeMeeting).toHaveBeenCalledWith("m-1", "b");
  expect(screen.getByRole("button", { name: "Summarize again" })).toBeInTheDocument();
});

test("another connection can be chosen and a failure is explained", async () => {
  vi.mocked(api.summarizeMeeting).mockRejectedValue(new api.LlmApiError("too_long", 422));
  render(<MeetingSummaryPanel meetingId="m-1" summary={written} meetingPrompt={null} connections={connections} defaultTemplate="T" />);

  fireEvent.change(screen.getByLabelText("Summarize with"), { target: { value: "a" } });
  fireEvent.click(screen.getByRole("button", { name: "Summarize again" }));

  expect(await screen.findByText(/too long for this connection/)).toBeInTheDocument();
  expect(api.summarizeMeeting).toHaveBeenCalledWith("m-1", "a");
  expect(screen.getByText(/Budget approved\./)).toBeInTheDocument();
});

test("the meeting's own prompt starts from the Settings one and can be cleared", async () => {
  vi.mocked(api.setMeetingPrompt).mockResolvedValue();
  render(<MeetingSummaryPanel meetingId="m-1" summary={null} meetingPrompt={null} connections={[]} defaultTemplate="Summarize {{title}}." />);

  expect(screen.getByText(/Add an AI connection in Settings/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Edit prompt for this meeting" }));
  const field = screen.getByLabelText("Prompt for this meeting");
  expect(field).toHaveValue("Summarize {{title}}.");
  fireEvent.change(field, { target: { value: "Only decisions." } });
  fireEvent.click(screen.getByRole("button", { name: "Save prompt" }));

  expect(await screen.findByText("Prompt saved for this meeting.")).toBeInTheDocument();
  expect(api.setMeetingPrompt).toHaveBeenCalledWith("m-1", "Only decisions.");
  fireEvent.click(screen.getByRole("button", { name: "Use the Settings prompt" }));
  expect(await screen.findByText("This meeting uses the Settings prompt again.")).toBeInTheDocument();
  expect(api.setMeetingPrompt).toHaveBeenLastCalledWith("m-1", null);
});

test("the summary can be edited, and written by hand without one", async () => {
  vi.mocked(edits.editSummaryText).mockResolvedValue({ ...written, text: "## Overview\nBudget approved on Monday.", edited: true });
  const { unmount } = render(
    <MeetingSummaryPanel meetingId="m-1" summary={written} meetingPrompt={null} connections={connections} defaultTemplate="T" />
  );

  fireEvent.click(screen.getByRole("button", { name: "Edit summary" }));
  const field = screen.getByLabelText("Summary (Markdown)");
  expect(field).toHaveValue("## Overview\nBudget approved.");
  fireEvent.change(field, { target: { value: "## Overview\nBudget approved on Monday." } });
  fireEvent.click(screen.getByRole("button", { name: "Save summary" }));

  expect(await screen.findByText("Summary saved.")).toBeInTheDocument();
  expect(edits.editSummaryText).toHaveBeenCalledWith("m-1", "## Overview\nBudget approved on Monday.");
  expect(screen.getByText(/Budget approved on Monday\./)).toBeInTheDocument();
  expect(screen.getByText(/, edited/)).toBeInTheDocument();
  unmount();

  vi.mocked(edits.editSummaryText).mockResolvedValue({ ...written, text: "Mine.", connection_label: "", model: "", edited: true });
  render(<MeetingSummaryPanel meetingId="m-1" summary={null} meetingPrompt={null} connections={[]} defaultTemplate="T" />);
  fireEvent.click(screen.getByRole("button", { name: "Write a summary" }));
  fireEvent.change(screen.getByLabelText("Summary (Markdown)"), { target: { value: "Mine." } });
  fireEvent.click(screen.getByRole("button", { name: "Save summary" }));
  expect(await screen.findByText("Written by you.")).toBeInTheDocument();
});
