// Verifies the meeting detail header renames its meeting from the pencil button.

import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingTitleHeader } from "@/components/meeting-title-header";
import * as api from "@/lib/api";

vi.mock("@/lib/api", async () => ({ ...(await vi.importActual<typeof api>("@/lib/api")), renameMeeting: vi.fn() }));

const meeting: api.MeetingSummary = { meeting_id: "meeting-titled", title: "Project Sync", created_at: "2026-10-27T09:05:00+00:00" };

beforeEach(() => {
  vi.mocked(api.renameMeeting).mockReset();
});

test("shows the title as the page heading with a pencil button beside it", () => {
  render(<MeetingTitleHeader meeting={meeting} />);

  const heading = screen.getByRole("heading", { level: 1, name: "Project Sync" });
  const pencil = screen.getByRole("button", { name: "Rename meeting" });
  expect(heading.parentElement).toContainElement(pencil);
});

test("the pencil opens the rename dialog pre-filled with the title", () => {
  render(<MeetingTitleHeader meeting={meeting} />);

  fireEvent.click(screen.getByRole("button", { name: "Rename meeting" }));

  const dialog = screen.getByRole("dialog", { name: "Rename meeting" });
  expect((within(dialog).getByLabelText("Title") as HTMLInputElement).value).toBe("Project Sync");
});

test("a successful rename updates the heading immediately", async () => {
  vi.mocked(api.renameMeeting).mockResolvedValue({ ...meeting, title: "Roadmap Review" });
  render(<MeetingTitleHeader meeting={meeting} />);
  fireEvent.click(screen.getByRole("button", { name: "Rename meeting" }));
  const dialog = screen.getByRole("dialog");

  fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Roadmap Review" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));

  expect(await screen.findByRole("heading", { level: 1, name: "Roadmap Review" })).toBeInTheDocument();
  expect(api.renameMeeting).toHaveBeenCalledWith("meeting-titled", "Roadmap Review");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent("Meeting renamed");
  expect(screen.getByRole("button", { name: "Rename meeting" })).toHaveFocus();
});

test("a failed rename keeps the old heading and shows the error in the dialog", async () => {
  vi.mocked(api.renameMeeting).mockRejectedValue(new api.MeetingRenameError("server", 500, null));
  render(<MeetingTitleHeader meeting={meeting} />);
  fireEvent.click(screen.getByRole("button", { name: "Rename meeting" }));
  const dialog = screen.getByRole("dialog");

  fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: "Roadmap Review" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));

  expect(await within(dialog).findByRole("alert")).toHaveTextContent("Couldn't save the new title. Try again.");
  expect(screen.getByRole("heading", { level: 1, name: "Project Sync" })).toBeInTheDocument();
});
