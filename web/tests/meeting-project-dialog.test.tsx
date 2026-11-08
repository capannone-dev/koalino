// Verifies adding a meeting to a project from anywhere (#94): the picker's "New project…" creates
// the project and moves the meeting in one step, and the meeting list's row menu opens the picker.

import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingList } from "@/components/meeting-list";
import { MeetingProjectPicker } from "@/components/meeting-project-picker";
import * as projectsApi from "@/lib/projects-api";

vi.mock("@/lib/projects-api", async () => ({
  ...(await vi.importActual<typeof projectsApi>("@/lib/projects-api")),
  createProject: vi.fn(),
  getProjects: vi.fn(),
  moveMeetingToProject: vi.fn()
}));

const hiring: projectsApi.ProjectSummary = {
  project_id: "p1",
  name: "Hiring",
  created_at: "2026-11-02T09:00:00+00:00",
  updated_at: "2026-11-02T09:00:00+00:00",
  meeting_count: 0,
  resource_count: 0,
  latest_meeting_at: null
} as projectsApi.ProjectSummary;

beforeEach(() => {
  vi.mocked(projectsApi.createProject).mockReset();
  vi.mocked(projectsApi.getProjects).mockReset();
  vi.mocked(projectsApi.moveMeetingToProject).mockReset();
});

test("New project… names a project, creates it and moves the meeting into it", async () => {
  vi.mocked(projectsApi.createProject).mockResolvedValue({ ...hiring, project_id: "p2", name: "Budget" });
  vi.mocked(projectsApi.moveMeetingToProject).mockResolvedValue(undefined);
  render(<MeetingProjectPicker meetingId="m1" projectId={null} projects={[hiring]} />);

  fireEvent.change(screen.getByLabelText("Project"), { target: { value: "__new__" } });
  fireEvent.change(screen.getByLabelText("New project name"), { target: { value: " Budget " } });
  fireEvent.click(screen.getByRole("button", { name: "Create and move" }));

  await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Moved to Budget"));
  expect(projectsApi.createProject).toHaveBeenCalledWith("Budget");
  expect(projectsApi.moveMeetingToProject).toHaveBeenCalledWith("m1", "p2");
  expect((screen.getByLabelText("Project") as HTMLSelectElement).value).toBe("p2");
});

test("an empty new project name is refused before anything is sent", () => {
  render(<MeetingProjectPicker meetingId="m1" projectId={null} projects={[hiring]} />);

  fireEvent.change(screen.getByLabelText("Project"), { target: { value: "__new__" } });
  fireEvent.click(screen.getByRole("button", { name: "Create and move" }));

  expect(screen.getByRole("alert")).toHaveTextContent(/name/i);
  expect(projectsApi.createProject).not.toHaveBeenCalled();
});

test("the meeting list's row menu adds a meeting to a project", async () => {
  vi.mocked(projectsApi.getProjects).mockResolvedValue([hiring]);
  vi.mocked(projectsApi.moveMeetingToProject).mockResolvedValue(undefined);
  render(<MeetingList meetings={[{ meeting_id: "m1", title: "Kick-off", created_at: "2026-11-02T09:00:00+00:00", project_id: null }]} />);

  fireEvent.click(screen.getByRole("button", { name: "Actions for Kick-off" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Add to project…" }));
  const dialog = await screen.findByRole("dialog", { name: "Add Kick-off to a project" });
  fireEvent.change(await within(dialog).findByLabelText("Project"), { target: { value: "p1" } });

  await waitFor(() => expect(within(dialog).getByRole("status")).toHaveTextContent("Moved to Hiring"));
  expect(projectsApi.moveMeetingToProject).toHaveBeenCalledWith("m1", "p1");
});
