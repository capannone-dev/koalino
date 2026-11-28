// Verifies the meeting list opens a rename dialog from its row menu and renames meetings in place.

import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingList } from "@/components/meeting-list";
import * as api from "@/lib/api";
import { formatMeetingWhen } from "@/lib/meeting-metadata";

vi.mock("@/lib/api", async () => ({ ...(await vi.importActual<typeof api>("@/lib/api")), deleteMeeting: vi.fn(), renameMeeting: vi.fn() }));

const meetings: api.MeetingSummary[] = [
  { meeting_id: "meeting-titled", title: "Project Sync", created_at: "2026-10-27T09:05:00+00:00" },
  { meeting_id: "meeting-untitled", title: "Meeting 2026-10-26 18:00 UTC", created_at: "2026-10-26T18:00:00+00:00" }
];

beforeEach(() => {
  vi.mocked(api.renameMeeting).mockReset();
});

/** Opens the rename dialog for "Project Sync" through its actions button and returns the dialog. */
function openRenameDialog(): HTMLElement {
  fireEvent.click(screen.getByRole("button", { name: "Actions for Project Sync" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Rename" }));
  return screen.getByRole("dialog", { name: "Rename meeting" });
}

/** Replaces the dialog's title and presses Save. */
function submitTitle(dialog: HTMLElement, newTitle: string): void {
  fireEvent.change(within(dialog).getByLabelText("Title"), { target: { value: newTitle } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));
}

test("lists each meeting title as a link to its detail page", () => {
  render(<MeetingList meetings={meetings} />);

  expect(screen.getByRole("link", { name: "Project Sync" })).toHaveAttribute("href", "/meetings/meeting-titled");
  expect(screen.getByRole("link", { name: "Meeting 2026-10-26 18:00 UTC" })).toBeInTheDocument();
});

test("rows show when the meeting happened in local words, not its internal ID (#210, #211)", () => {
  const { container } = render(<MeetingList meetings={meetings} />);
  expect(screen.queryByText("meeting-titled")).not.toBeInTheDocument();
  const times = container.querySelectorAll("time");
  expect(times[0]).toHaveAttribute("dateTime", "2026-10-27T09:05:00+00:00");
  expect(times[0].textContent).toBe(formatMeetingWhen("2026-10-27T09:05:00+00:00", "en"));
});

test("the summary link says Summary in words, not only with an icon", () => {
  render(<MeetingList meetings={meetings} />);

  const summaryLink = screen.getByRole("link", { name: "Summary of Project Sync" });
  expect(summaryLink).toHaveAttribute("href", "/meetings/meeting-titled#summary");
  expect(summaryLink).toHaveTextContent("Summary");
});

test("the actions button opens a menu with Rename", () => {
  render(<MeetingList meetings={meetings} />);
  const actionsButton = screen.getByRole("button", { name: "Actions for Project Sync" });
  expect(actionsButton).toHaveAttribute("aria-expanded", "false");

  fireEvent.click(actionsButton);

  expect(actionsButton).toHaveAttribute("aria-expanded", "true");
  expect(screen.getByRole("menu")).toBeInTheDocument();
  expect(screen.getByRole("menuitem", { name: "Rename" })).toHaveFocus();
});

test("right-clicking a row opens the same menu", () => {
  render(<MeetingList meetings={meetings} />);

  const notPrevented = fireEvent.contextMenu(screen.getByRole("link", { name: "Project Sync" }));

  expect(notPrevented).toBe(false);
  expect(screen.getByRole("menuitem", { name: "Rename" })).toBeInTheDocument();
});

test("Escape closes the menu and returns focus to the actions button", () => {
  render(<MeetingList meetings={meetings} />);
  const actionsButton = screen.getByRole("button", { name: "Actions for Project Sync" });
  fireEvent.click(actionsButton);

  fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });

  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  expect(actionsButton).toHaveFocus();
});

test("Rename opens a modal pre-filled with the current title", () => {
  render(<MeetingList meetings={meetings} />);

  const dialog = openRenameDialog();

  expect(dialog).toHaveAttribute("aria-modal", "true");
  const titleInput = within(dialog).getByLabelText("Title") as HTMLInputElement;
  expect(titleInput.value).toBe("Project Sync");
  expect(titleInput).toHaveFocus();
  expect(titleInput.selectionStart).toBe(0);
  expect(titleInput.selectionEnd).toBe(12);
  expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeInTheDocument();
  expect(within(dialog).getByRole("button", { name: "Save" })).toBeInTheDocument();
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
});

test.each([
  ["   ", "Enter a title."],
  ["a".repeat(201), "Titles can be at most 200 characters (you have 201)."],
  ["Weekly\tsync", "Titles can't contain line breaks or control characters."]
])("the invalid title %j shows %j and is not sent", (newTitle, message) => {
  render(<MeetingList meetings={meetings} />);
  const dialog = openRenameDialog();

  submitTitle(dialog, newTitle);

  expect(within(dialog).getByRole("alert")).toHaveTextContent(message);
  expect(within(dialog).getByLabelText("Title")).toHaveAttribute("aria-invalid", "true");
  expect(api.renameMeeting).not.toHaveBeenCalled();
});

test("a successful rename sends the trimmed title, closes the dialog, and updates the row", async () => {
  vi.mocked(api.renameMeeting).mockResolvedValue({ ...meetings[0], title: "Roadmap Review" });
  render(<MeetingList meetings={meetings} />);
  const dialog = openRenameDialog();

  submitTitle(dialog, "  Roadmap Review  ");

  expect(await screen.findByRole("link", { name: "Roadmap Review" })).toBeInTheDocument();
  expect(api.renameMeeting).toHaveBeenCalledWith("meeting-titled", "Roadmap Review");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.queryByText("Project Sync")).not.toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent("Meeting renamed");
  expect(screen.getByRole("button", { name: "Actions for Roadmap Review" })).toHaveFocus();
});

test("pressing Enter in the field saves the title", async () => {
  vi.mocked(api.renameMeeting).mockResolvedValue({ ...meetings[0], title: "Roadmap Review" });
  render(<MeetingList meetings={meetings} />);
  const dialog = openRenameDialog();
  const titleInput = within(dialog).getByLabelText("Title");

  fireEvent.change(titleInput, { target: { value: "Roadmap Review" } });
  fireEvent.submit(titleInput);

  expect(await screen.findByRole("link", { name: "Roadmap Review" })).toBeInTheDocument();
});

test("saving an unchanged title closes the dialog without a request", () => {
  render(<MeetingList meetings={meetings} />);
  const dialog = openRenameDialog();

  submitTitle(dialog, " Project Sync ");

  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(api.renameMeeting).not.toHaveBeenCalled();
});

test("Cancel and Escape close the dialog without renaming and restore focus", () => {
  render(<MeetingList meetings={meetings} />);
  const actionsButton = screen.getByRole("button", { name: "Actions for Project Sync" });

  submitTitleThenCancel();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(actionsButton).toHaveFocus();

  const dialog = openRenameDialog();
  fireEvent.keyDown(dialog, { key: "Escape" });
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(actionsButton).toHaveFocus();
  expect(api.renameMeeting).not.toHaveBeenCalled();
  expect(screen.getByRole("link", { name: "Project Sync" })).toBeInTheDocument();

  /** Types a new title, then presses Cancel instead of Save. */
  function submitTitleThenCancel(): void {
    const cancelDialog = openRenameDialog();
    fireEvent.change(within(cancelDialog).getByLabelText("Title"), { target: { value: "Roadmap Review" } });
    fireEvent.click(within(cancelDialog).getByRole("button", { name: "Cancel" }));
  }
});

test.each([
  ["invalid_title", 400, "This title isn't allowed."],
  ["not_found", 404, "This meeting no longer exists."],
  ["server", 500, "Couldn't save the new title. Try again."],
  ["network", null, "Can't reach the note-taking server. Check that it is running."],
  ["unexpected", 422, "Couldn't rename the meeting."]
] as const)("a %s failure keeps the dialog open with a message", async (kind, status, message) => {
  vi.mocked(api.renameMeeting).mockRejectedValue(new api.MeetingRenameError(kind, status, "server detail"));
  render(<MeetingList meetings={meetings} />);
  const dialog = openRenameDialog();

  submitTitle(dialog, "Roadmap Review");

  expect(await within(dialog).findByRole("alert")).toHaveTextContent(message);
  expect(screen.getByRole("dialog")).toBeInTheDocument();
  expect(screen.queryByText("server detail")).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Project Sync" })).toBeInTheDocument();
  expect(within(dialog).getByRole("button", { name: "Save" })).toBeEnabled();
});

test("Save is disabled while the rename is in flight", async () => {
  let resolveRename: (summary: api.MeetingSummary) => void = () => {};
  vi.mocked(api.renameMeeting).mockReturnValue(new Promise((resolve) => (resolveRename = resolve)));
  render(<MeetingList meetings={meetings} />);
  const dialog = openRenameDialog();

  submitTitle(dialog, "Roadmap Review");

  expect(within(dialog).getByRole("button", { name: "Saving…" })).toBeDisabled();
  fireEvent.click(within(dialog).getByRole("button", { name: "Saving…" }));
  expect(api.renameMeeting).toHaveBeenCalledTimes(1);
  resolveRename({ ...meetings[0], title: "Roadmap Review" });
  expect(await screen.findByRole("link", { name: "Roadmap Review" })).toBeInTheDocument();
});

test("an empty list says there are no meetings", () => {
  render(<MeetingList meetings={[]} />);

  expect(screen.getByText("No meetings yet.")).toBeInTheDocument();
  expect(screen.getByText("Meetings recorded with Koalino on your phone or computer appear here once they sync.")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Pair a device" })).toHaveAttribute("href", "/devices");
});

test("Delete in the row menu asks first, then removes only that row", async () => {
  const deleted = vi.mocked(api.deleteMeeting).mockResolvedValue();
  render(<MeetingList meetings={meetings} />);

  fireEvent.click(screen.getByRole("button", { name: "Actions for Project Sync" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
  expect(screen.getByRole("alertdialog", { name: "Delete Project Sync?" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Delete" }));

  expect(await screen.findByText("Meeting deleted")).toBeInTheDocument();
  expect(deleted).toHaveBeenCalledWith("meeting-titled");
  expect(screen.queryByText("Project Sync")).not.toBeInTheDocument();
  expect(screen.getByText("Meeting 2026-10-26 18:00 UTC")).toBeInTheDocument();
  deleted.mockReset();
});

test("a meeting being recorded stays, and the dialog says why", async () => {
  const deleted = vi.mocked(api.deleteMeeting).mockRejectedValue(new api.DeletionError("recording"));
  render(<MeetingList meetings={meetings} />);

  fireEvent.click(screen.getByRole("button", { name: "Actions for Project Sync" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
  fireEvent.click(screen.getByRole("button", { name: "Delete" }));

  expect(await screen.findByText(/being recorded/)).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Project Sync" })).toBeInTheDocument();
  deleted.mockReset();
});

test("a combined meeting's row names its devices (#88)", () => {
  render(
    <MeetingList
      meetings={[
        {
          meeting_id: "meeting-laptop",
          title: "Supplier call",
          created_at: "2026-11-02T15:00:00+00:00",
          recorded_on: [
            { device_id: "device-laptop", name: "Laptop", this_device: true },
            { device_id: "device-phone", name: "Pixel", this_device: false }
          ],
          combined_recordings: ["meeting-phone"]
        }
      ]}
    />
  );

  expect(screen.getByText("Recorded on Laptop + Pixel")).toBeInTheDocument();
});
