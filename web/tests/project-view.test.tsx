// Verifies the project pages (Issue #85): creating a project, listing a project's meetings and
// files, adding a meeting, previewing an e-mail, uploading, deleting a file, and the meeting page's
// project picker.

import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingProjectPicker } from "@/components/meeting-project-picker";
import { ProjectView } from "@/components/project-view";
import { ProjectsIndex } from "@/components/projects-index";
import type { MeetingSummary } from "@/lib/api";
import * as projectsApi from "@/lib/projects-api";

const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push, refresh: vi.fn() }) }));
// The project page also holds its chat (#97), which is not under test here.
vi.mock("@/lib/chat-api", async () => ({
  ...(await vi.importActual<typeof import("@/lib/chat-api")>("@/lib/chat-api")),
  getChat: vi.fn().mockResolvedValue([])
}));
vi.mock("@/lib/projects-api", async () => ({
  ...(await vi.importActual<typeof projectsApi>("@/lib/projects-api")),
  createProject: vi.fn(),
  getProject: vi.fn(),
  getProjectEmail: vi.fn(),
  moveMeetingToProject: vi.fn(),
  moveProject: vi.fn(),
  uploadProjectFile: vi.fn(),
  deleteProjectResource: vi.fn()
}));

const summary: projectsApi.ProjectSummary = {
  project_id: "p1",
  name: "Hiring",
  created_at: "2026-11-01T09:00:00+00:00",
  updated_at: "2026-11-01T09:00:00+00:00",
  meeting_count: 1,
  resource_count: 2,
  latest_meeting_at: null
};

const detail: projectsApi.ProjectDetail = {
  project: summary,
  meetings: [{ meeting_id: "m1", title: "Kick-off", created_at: "2026-11-01T09:00:00+00:00", duration_ms: 0, project_id: "p1" }],
  resources: [
    { attachment_id: "r-mail", file_name: "offer.eml", mime_type: "message/rfc822", kind: "email", added_at: "", size_bytes: 2048 },
    { attachment_id: "r-pdf", file_name: "cv.pdf", mime_type: "application/pdf", kind: "pdf", added_at: "", size_bytes: null }
  ]
};

const meetings: MeetingSummary[] = [
  { meeting_id: "m1", title: "Kick-off", created_at: "", project_id: "p1" },
  { meeting_id: "m2", title: "Budget review", created_at: "", project_id: null }
];

beforeEach(() => {
  vi.mocked(projectsApi.getProject).mockReset().mockResolvedValue(detail);
  push.mockReset();
});

test("the index lists projects with counts and creates one, then opens it", async () => {
  vi.mocked(projectsApi.createProject).mockResolvedValue({ ...summary, project_id: "p2", name: "Offers" });
  render(<ProjectsIndex projects={[summary]} />);

  expect(screen.getByRole("link", { name: /Hiring/ })).toHaveAttribute("href", "/projects/p1");
  expect(screen.getByText("1 meeting · 2 files")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Create" }));
  expect(screen.getByRole("alert")).toHaveTextContent("Enter a name.");
  expect(projectsApi.createProject).not.toHaveBeenCalled();

  fireEvent.change(screen.getByLabelText("New project"), { target: { value: " Offers " } });
  fireEvent.click(screen.getByRole("button", { name: "Create" }));
  await waitFor(() => expect(push).toHaveBeenCalledWith("/projects/p2"));
  expect(projectsApi.createProject).toHaveBeenCalledWith("Offers");
});

test("the project page lists meetings and files; a file still syncing can't be opened", () => {
  render(<ProjectView initialDetail={detail} allMeetings={meetings} />);

  expect(screen.getByRole("link", { name: "Kick-off" })).toHaveAttribute("href", "/meetings/m1");
  expect(screen.getByText("offer.eml")).toBeInTheDocument();
  expect(screen.getByText("email · 2 KB")).toBeInTheDocument();
  expect(screen.getByText("pdf · Syncing…")).toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "Open cv.pdf" })).toBeNull();
  expect(screen.getByRole("link", { name: "Download offer.eml" })).toHaveAttribute("href", "http://127.0.0.1:8080/api/projects/p1/resources/r-mail/file");
});

test("only meetings outside projects can be added", async () => {
  vi.mocked(projectsApi.moveMeetingToProject).mockResolvedValue(undefined);
  render(<ProjectView initialDetail={detail} allMeetings={meetings} />);

  const select = screen.getByLabelText("Add a meeting");
  expect(within(select).getAllByRole("option").map((option) => option.textContent)).toEqual(["Choose a meeting…", "Budget review"]);
  fireEvent.change(select, { target: { value: "m2" } });
  fireEvent.click(screen.getByRole("button", { name: "Add" }));

  await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Meeting added"));
  expect(projectsApi.moveMeetingToProject).toHaveBeenCalledWith("m2", "p1");
});

test("an e-mail opens as headers and text, without the server's markup", async () => {
  vi.mocked(projectsApi.getProjectEmail).mockResolvedValue({
    subject: "Your offer",
    from: [{ name: "Anna", address: "anna@example.com" }],
    to: [],
    cc: [],
    sent_at: null,
    body_text: "<b>I accept.</b>",
    body_truncated: false,
    attachments: [{ file_name: "contract.pdf", mime_type: "application/pdf", size_bytes: 10 }]
  });
  render(<ProjectView initialDetail={detail} allMeetings={meetings} />);

  fireEvent.click(screen.getByRole("button", { name: "View offer.eml" }));

  expect(await screen.findByRole("heading", { name: "Your offer" })).toBeInTheDocument();
  expect(screen.getByText("Anna <anna@example.com>")).toBeInTheDocument();
  expect(screen.getByText("<b>I accept.</b>")).toBeInTheDocument();
  expect(screen.getByText("Attachments: contract.pdf")).toBeInTheDocument();
});

test("uploading reports what was added", async () => {
  vi.mocked(projectsApi.uploadProjectFile).mockResolvedValue({ added: [detail.resources[0]], skipped_attachments: 1 });
  const { container } = render(<ProjectView initialDetail={detail} allMeetings={meetings} />);

  const input = container.querySelector('input[type="file"]') as HTMLInputElement;
  fireEvent.change(input, { target: { files: [new File(["x"], "offer.eml")] } });

  await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Added 1 file; 1 e-mail attachments were left in the e-mail"));
});

test("deleting a file asks first, then reloads the project", async () => {
  vi.mocked(projectsApi.deleteProjectResource).mockResolvedValue(undefined);
  vi.mocked(projectsApi.getProject).mockResolvedValue({ ...detail, resources: [detail.resources[1]] });
  render(<ProjectView initialDetail={detail} allMeetings={meetings} />);

  fireEvent.click(screen.getByRole("button", { name: "Delete offer.eml" }));
  const dialog = screen.getByRole("alertdialog");
  fireEvent.click(within(dialog).getByRole("button", { name: "Delete" }));

  await waitFor(() => expect(screen.queryByText("offer.eml")).toBeNull());
  expect(projectsApi.deleteProjectResource).toHaveBeenCalledWith("p1", "r-mail");
});

test("the meeting page's picker moves the meeting and restores the choice on failure", async () => {
  vi.mocked(projectsApi.moveMeetingToProject).mockRejectedValueOnce(new projectsApi.ProjectApiError("project_not_found", 404)).mockResolvedValueOnce(undefined);
  render(<MeetingProjectPicker meetingId="m2" projectId={null} projects={[summary]} />);
  const select = screen.getByLabelText("Project") as HTMLSelectElement;

  fireEvent.change(select, { target: { value: "p1" } });
  await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("This project no longer exists."));
  expect(select.value).toBe("");

  fireEvent.change(select, { target: { value: "p1" } });
  await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Moved to Hiring"));
  expect(select.value).toBe("p1");
});

/** Hiring (p1) holds Interviews (p2), which holds Round two (p3); Budget (p4) is apart (#142). */
const nested: projectsApi.ProjectSummary[] = [
  { ...summary, parent_id: null },
  { ...summary, project_id: "p2", name: "Interviews", parent_id: "p1" },
  { ...summary, project_id: "p3", name: "Round two", parent_id: "p2" },
  { ...summary, project_id: "p4", name: "Budget", parent_id: null }
];

test("the index shows subprojects indented under their parent (#142)", () => {
  render(<ProjectsIndex projects={nested} />);

  const links = screen.getAllByRole("link").map((link) => [link.textContent?.split("1 meeting")[0], link.getAttribute("data-depth")]);
  expect(links).toEqual([
    ["Budget", "0"],
    ["Hiring", "0"],
    ["Interviews", "1"],
    ["Round two", "2"]
  ]);
});

test("a project page lists its subprojects and creates one inside it (#142)", async () => {
  vi.mocked(projectsApi.createProject).mockResolvedValue({ ...summary, project_id: "p9", name: "Offers", parent_id: "p1" });
  render(<ProjectView initialDetail={detail} allMeetings={meetings} allProjects={nested} />);

  const subprojects = screen.getByRole("region", { name: "Subprojects" });
  expect(within(subprojects).getByRole("link", { name: /Interviews/ })).toHaveAttribute("href", "/projects/p2");
  expect(within(subprojects).queryByText("Round two")).toBeNull();

  fireEvent.change(within(subprojects).getByLabelText("New subproject"), { target: { value: "Offers" } });
  fireEvent.click(within(subprojects).getByRole("button", { name: "Create" }));
  await waitFor(() => expect(push).toHaveBeenCalledWith("/projects/p9"));
  expect(projectsApi.createProject).toHaveBeenCalledWith("Offers", "p1");
});

test("Move to offers the top level and other projects, never the project or its subprojects (#142)", async () => {
  vi.mocked(projectsApi.moveProject).mockResolvedValue({ ...summary, parent_id: "p4" });
  render(<ProjectView initialDetail={detail} allMeetings={meetings} allProjects={nested} />);

  const select = screen.getByLabelText("Move to");
  const options = within(select).getAllByRole("option").map((option) => option.textContent?.trim());
  expect(options).toEqual(["Top level", "Budget"]);

  fireEvent.change(select, { target: { value: "p4" } });
  fireEvent.click(screen.getByRole("button", { name: "Move" }));
  await waitFor(() => expect(projectsApi.moveProject).toHaveBeenCalledWith("p1", "p4"));
  expect(await screen.findByText("Project moved")).toBeInTheDocument();
});
