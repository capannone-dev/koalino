// Verifies the web Settings page's notes folder (#98): the suggested folder while off, turning it
// on with a folder, Sync now, and a refused folder worded.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { NotesFolderSettings } from "@/components/notes-folder-settings";
import * as api from "@/lib/notes-folder-api";

vi.mock("@/lib/notes-folder-api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/notes-folder-api")),
  setNotesFolder: vi.fn(),
  syncNotesFolder: vi.fn()
}));

const off: api.NotesFolderStatus = {
  enabled: false,
  path: null,
  suggested_path: "/home/anna/NoteTaker",
  last_round_at: null,
  last_error: null,
  written: 0,
  imported: 0
};

beforeEach(() => {
  vi.mocked(api.setNotesFolder).mockReset();
  vi.mocked(api.syncNotesFolder).mockReset();
});

test("turning it on saves the folder and shows the round", async () => {
  vi.mocked(api.setNotesFolder).mockResolvedValue({
    ...off,
    enabled: true,
    path: "/home/anna/Notes",
    last_round_at: "2026-11-09T12:00:00Z",
    written: 7
  });
  render(<NotesFolderSettings initial={off} />);
  expect(screen.getByLabelText("Folder on the server")).toHaveValue("/home/anna/NoteTaker");
  expect(screen.getByText(/decrypted copy/)).toBeInTheDocument();
  expect(screen.queryByText("Sync now")).not.toBeInTheDocument();

  fireEvent.change(screen.getByLabelText("Folder on the server"), { target: { value: "/home/anna/Notes" } });
  fireEvent.click(screen.getByRole("switch"));

  expect(await screen.findByText(/7 files written/)).toBeInTheDocument();
  expect(api.setNotesFolder).toHaveBeenCalledWith(true, "/home/anna/Notes");
  expect(screen.getByRole("switch")).toBeChecked();

  vi.mocked(api.syncNotesFolder).mockResolvedValue({
    ...off,
    enabled: true,
    path: "/home/anna/Notes",
    last_round_at: "2026-11-09T12:01:00Z",
    imported: 2
  });
  fireEvent.click(screen.getByText("Sync now"));
  expect(await screen.findByText(/2 edits and files taken in/)).toBeInTheDocument();
});

test("a refused folder is worded and the switch stays off", async () => {
  vi.mocked(api.setNotesFolder).mockRejectedValue(new api.NotesFolderError("invalid_folder", 400));
  render(<NotesFolderSettings initial={off} />);

  fireEvent.click(screen.getByRole("switch"));

  expect(await screen.findByText(/Choose a full folder path/)).toBeInTheDocument();
  expect(screen.getByRole("switch")).not.toBeChecked();
});

test("without the server nothing can be changed", () => {
  render(<NotesFolderSettings initial={null} />);
  expect(screen.getByRole("switch")).toBeDisabled();
  expect(screen.getByText(/Can't read the notes folder setting/)).toBeInTheDocument();
});
