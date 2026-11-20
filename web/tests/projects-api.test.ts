// Verifies the projects client (Issue #85) calls the documented routes and maps failures to kinds.

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import {
  addProjectNote,
  createProject,
  deleteProjectResource,
  getProjectResourceText,
  isWithinProject,
  moveMeetingToProject,
  moveProject,
  projectTree,
  ProjectApiError,
  projectErrorMessage,
  projectNameError,
  projectResourceFileUrl,
  uploadProjectFile
} from "@/lib/projects-api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const summary = { project_id: "p1", name: "Hiring", created_at: "", updated_at: "", meeting_count: 0, resource_count: 0, latest_meeting_at: null };

test("createProject posts the name", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify(summary), { status: 201 }));

  await expect(createProject("Hiring")).resolves.toEqual(summary);
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/projects", {
    cache: "no-store",
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ name: "Hiring" })
  });
});

test("moveMeetingToProject puts the project, or null to move the meeting out", async () => {
  fetchMock.mockResolvedValue(new Response(null, { status: 204 }));

  await expect(moveMeetingToProject("meeting 1", null)).resolves.toBeUndefined();
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/meetings/meeting%201/project", expect.objectContaining({ method: "PUT", body: JSON.stringify({ project_id: null }) }));
});

test("uploadProjectFile sends the raw file with its name in the query", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ added: [], skipped_attachments: 0 }), { status: 201 }));
  const file = new File(["From: a@b"], "offer & terms.eml", { type: "message/rfc822" });

  await uploadProjectFile("p1", file);

  const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
  expect(url).toBe("http://127.0.0.1:8080/api/projects/p1/resources?name=offer%20%26%20terms.eml");
  expect(init.method).toBe("POST");
  expect(init.body).toBe(file);
});

test("addProjectNote posts the title and text", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ attachment_id: "r1" }), { status: 201 }));

  await addProjectNote("p1", "Plan", "# Steps");

  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/projects/p1/notes", expect.objectContaining({ body: JSON.stringify({ title: "Plan", text: "# Steps" }) }));
});

test("a failure carries the daemon's error code, never its text", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ error_code: "resource_not_found", error: "internal detail" }), { status: 404 }));

  const failure = await deleteProjectResource("p1", "r1").catch((error: unknown) => error);

  expect(failure).toBeInstanceOf(ProjectApiError);
  expect((failure as ProjectApiError).kind).toBe("resource_not_found");
  expect(projectErrorMessage(failure)).toBe("That file is gone, or hasn't synced to this device yet.");
  expect(projectErrorMessage(failure)).not.toContain("internal detail");
});

test("an unknown code, a 503 and an unreachable daemon map to their kinds", async () => {
  fetchMock.mockResolvedValueOnce(new Response(JSON.stringify({ error_code: "surprise" }), { status: 400 }));
  await expect(createProject("x")).rejects.toMatchObject({ kind: "unexpected" });
  fetchMock.mockResolvedValueOnce(new Response("", { status: 503 }));
  await expect(createProject("x")).rejects.toMatchObject({ kind: "unavailable" });
  fetchMock.mockRejectedValueOnce(new TypeError("fetch failed"));
  await expect(createProject("x")).rejects.toMatchObject({ kind: "network" });
});

test("resource text is read from the file route", async () => {
  fetchMock.mockResolvedValue(new Response("# Notes", { status: 200 }));

  await expect(getProjectResourceText("p1", "r1")).resolves.toBe("# Notes");
  expect(fetchMock.mock.calls[0][0]).toBe(projectResourceFileUrl("p1", "r1"));
  expect(projectResourceFileUrl("p 1", "r1")).toBe("http://127.0.0.1:8080/api/projects/p%201/resources/r1/file");
});

test("project names follow Rust's rules", () => {
  expect(projectNameError("  Hiring ")).toBeNull();
  expect(projectNameError("   ")).toBe("Enter a name.");
  expect(projectNameError("a".repeat(201))).toBe("Names can be at most 200 characters.");
  expect(projectNameError("two\nlines")).toBe("Names can't contain line breaks or control characters.");
});

const project = (project_id: string, name: string, parent_id: string | null = null) => ({ ...summary, project_id, name, parent_id });

test("createProject sends the parent of a subproject (#142)", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify(summary), { status: 201 }));

  await createProject("Interviews", "p1");
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/projects", expect.objectContaining({ body: JSON.stringify({ name: "Interviews", parent_id: "p1" }) }));
});

test("moveProject puts the parent, or null for the top level, and names a loop (#142)", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify(summary), { status: 200 }));
  await moveProject("p 2", null);
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/projects/p%202/parent", expect.objectContaining({ method: "PUT", body: JSON.stringify({ parent_id: null }) }));

  fetchMock.mockResolvedValue(new Response(JSON.stringify({ error_code: "invalid_parent", error: "x" }), { status: 409 }));
  const failure = await moveProject("p1", "p2").catch((error: unknown) => error);
  expect(failure).toBeInstanceOf(ProjectApiError);
  expect(projectErrorMessage(failure)).toBe("A project can't move inside itself or its subprojects.");
});

test("projectTree orders projects under their parents, unknown parents at the top (#142)", () => {
  const projects = [project("p3", "Round two", "p2"), project("p4", "Budget"), project("p2", "Interviews", "p1"), project("p1", "Hiring"), project("p5", "Orphan", "gone")];

  expect(projectTree(projects).map(({ project, depth }) => `${depth}:${project.name}`)).toEqual(["0:Budget", "0:Hiring", "1:Interviews", "2:Round two", "0:Orphan"]);
  expect(isWithinProject(projects, "p3", "p1")).toBe(true);
  expect(isWithinProject(projects, "p1", "p3")).toBe(false);
  expect(isWithinProject(projects, "p1", "p1")).toBe(true);
});
