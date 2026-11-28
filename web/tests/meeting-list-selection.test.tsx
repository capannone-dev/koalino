// Specifies selecting several meetings in the list (#216): "Select" in a row's menu starts it,
// checkboxes add rows, and the bar deletes or moves them all; Escape or ✕ leaves it.

import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingList } from "@/components/meeting-list";
import * as api from "@/lib/api";
import * as projectsApi from "@/lib/projects-api";

vi.mock("@/lib/api", async () => ({ ...(await vi.importActual<typeof api>("@/lib/api")), deleteMeeting: vi.fn() }));
vi.mock("@/lib/projects-api", async () => ({
  ...(await vi.importActual<typeof projectsApi>("@/lib/projects-api")),
  getProjects: vi.fn(),
  moveMeetingToProject: vi.fn(),
}));

const meetings: api.MeetingSummary[] = [
  { meeting_id: "m1", title: "Project Sync", created_at: "2026-10-27T09:05:00+00:00" },
  { meeting_id: "m2", title: "Budget review", created_at: "2026-10-26T18:00:00+00:00" },
  { meeting_id: "m3", title: "Retro", created_at: "2026-10-25T18:00:00+00:00" },
];

beforeEach(() => {
  vi.mocked(api.deleteMeeting).mockReset().mockResolvedValue(undefined);
  vi.mocked(projectsApi.moveMeetingToProject).mockReset().mockResolvedValue(undefined as never);
  vi.mocked(projectsApi.getProjects).mockReset().mockResolvedValue([
    { project_id: "p1", name: "Launch", parent_id: null } as unknown as projectsApi.ProjectSummary,
  ]);
});

/** Starts a selection with `title` through its row menu. */
function selectFromMenu(title: string) {
  fireEvent.click(screen.getByRole("button", { name: `Actions for ${title}` }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Select" }));
}

test("Select starts a selection; checkboxes add rows; Delete removes them all", async () => {
  render(<MeetingList meetings={meetings} />);
  selectFromMenu("Project Sync");
  const bar = screen.getByRole("toolbar", { name: "1 selected" });
  expect(screen.getByRole("checkbox", { name: "Select Project Sync" })).toBeChecked();
  fireEvent.click(screen.getByRole("checkbox", { name: "Select Budget review" }));
  expect(within(screen.getByRole("toolbar")).getByText("2 selected")).toBeInTheDocument();
  expect(bar).toBeInTheDocument();

  fireEvent.click(within(screen.getByRole("toolbar")).getByRole("button", { name: "Delete" }));
  const dialog = screen.getByRole("alertdialog", { name: "Delete 2 meetings?" });
  await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "Delete" })));

  expect(vi.mocked(api.deleteMeeting).mock.calls.map(([id]) => id)).toEqual(["m1", "m2"]);
  expect(screen.queryByText("Project Sync")).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Retro" })).toBeInTheDocument();
  expect(screen.getByRole("status")).toHaveTextContent("2 meetings deleted");
  expect(screen.queryByRole("toolbar")).not.toBeInTheDocument();
});

test("Move to project moves every selected meeting", async () => {
  render(<MeetingList meetings={meetings} />);
  selectFromMenu("Project Sync");
  fireEvent.click(screen.getByRole("button", { name: "Select all" }));
  fireEvent.click(screen.getByRole("button", { name: "Move to project" }));
  const dialog = await screen.findByRole("dialog", { name: "Move to project" });
  await waitFor(() => expect(within(dialog).getByRole("button", { name: "Move" })).toBeEnabled());
  await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "Move" })));

  expect(vi.mocked(projectsApi.moveMeetingToProject).mock.calls).toEqual([
    ["m1", "p1"],
    ["m2", "p1"],
    ["m3", "p1"],
  ]);
  expect(screen.getByRole("status")).toHaveTextContent("Moved 3 meetings to Launch");
});

test("Escape and ✕ leave the selection", () => {
  render(<MeetingList meetings={meetings} />);
  selectFromMenu("Retro");
  fireEvent.keyDown(document.body, { key: "Escape" });
  expect(screen.queryByRole("toolbar")).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Retro" })).toBeInTheDocument();

  selectFromMenu("Retro");
  fireEvent.click(screen.getByRole("button", { name: "Cancel selection" }));
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
});
