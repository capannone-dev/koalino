// Verifies a meeting's notes on its page (#96): shown as written, edited in a text field, saved,
// and failures explained.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingNotes } from "@/components/meeting-notes";
import * as edits from "@/lib/edits-api";

vi.mock("@/lib/edits-api", async () => ({
  ...(await vi.importActual<typeof edits>("@/lib/edits-api")),
  saveMeetingNotes: vi.fn()
}));

beforeEach(() => vi.mocked(edits.saveMeetingNotes).mockReset());

test("notes are written, saved and shown", async () => {
  vi.mocked(edits.saveMeetingNotes).mockResolvedValue();
  render(<MeetingNotes meetingId="m-1" notes={null} />);

  expect(screen.getByText("No notes yet.")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Write notes" }));
  fireEvent.change(screen.getByLabelText("Notes (Markdown)"), { target: { value: "## Actions\n- Call the courier" } });
  fireEvent.click(screen.getByRole("button", { name: "Save notes" }));

  expect(await screen.findByText("Notes saved.")).toBeInTheDocument();
  expect(edits.saveMeetingNotes).toHaveBeenCalledWith("m-1", "## Actions\n- Call the courier");
  expect(screen.getByText(/Call the courier/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Edit notes" })).toBeInTheDocument();
});

test("a failed save keeps the text being edited", async () => {
  vi.mocked(edits.saveMeetingNotes).mockRejectedValueOnce(new edits.EditApiError("too_long", 400));
  render(<MeetingNotes meetingId="m-1" notes="Agenda" />);

  fireEvent.click(screen.getByRole("button", { name: "Edit notes" }));
  fireEvent.change(screen.getByLabelText("Notes (Markdown)"), { target: { value: "Agenda and more" } });
  fireEvent.click(screen.getByRole("button", { name: "Save notes" }));

  expect(await screen.findByText("The text is too long.")).toBeInTheDocument();
  expect(screen.getByLabelText("Notes (Markdown)")).toHaveValue("Agenda and more");
});
