// Typed client for the daemon's project routes (Issue #85): projects as folders of meetings with
// their files, notes and e-mails. Responses never carry local paths; files are fetched by URL.

import { apiBaseUrl } from "@/lib/api";
import { maxMeetingTitleCharacters, validateMeetingTitle } from "@/lib/meeting-title";

/** A project list entry; times are RFC 3339, `latest_meeting_at` null without meetings. */
export type ProjectSummary = {
  project_id: string;
  name: string;
  created_at: string;
  updated_at: string;
  meeting_count: number;
  resource_count: number;
  latest_meeting_at: string | null;
};

/** How the dashboard shows a resource, decided by Rust from its MIME type. */
export type ResourceKind = "image" | "markdown" | "pdf" | "email" | "text" | "document";

/** A file, note or e-mail in a project. `size_bytes` is null while it is still syncing here. */
export type ProjectResource = {
  attachment_id: string;
  file_name: string;
  mime_type: string;
  kind: ResourceKind;
  added_at: string;
  size_bytes: number | null;
};

/** A meeting in a project. */
export type ProjectMeeting = { meeting_id: string; title: string; created_at: string; duration_ms: number; project_id: string };

/** `GET /api/projects/{id}`. */
export type ProjectDetail = { project: ProjectSummary; meetings: ProjectMeeting[]; resources: ProjectResource[] };

/** A person in an e-mail's headers. */
export type EmailPerson = { name: string | null; address: string | null };

/** An e-mail resource read by Rust; HTML bodies arrive as plain text. */
export type EmailView = {
  subject: string | null;
  from: EmailPerson[];
  to: EmailPerson[];
  cc: EmailPerson[];
  sent_at: string | null;
  body_text: string;
  body_truncated: boolean;
  attachments: { file_name: string; mime_type: string; size_bytes: number }[];
};

/** `POST /api/projects/{id}/resources`: what was added (an e-mail adds its attachments too). */
export type AddedResources = { added: ProjectResource[]; skipped_attachments: number };

/** Why a project request failed: the daemon's `error_code`, or `network` when it was unreachable. */
export type ProjectFailureKind =
  | "invalid_request"
  | "invalid_name"
  | "project_not_found"
  | "meeting_not_found"
  | "resource_not_found"
  | "not_an_email"
  | "rejected"
  | "storage_failed"
  | "unavailable"
  | "network"
  | "unexpected";

const knownKinds: readonly ProjectFailureKind[] = [
  "invalid_request",
  "invalid_name",
  "project_not_found",
  "meeting_not_found",
  "resource_not_found",
  "not_an_email",
  "rejected",
  "storage_failed",
  "unavailable"
];

/** Thrown by every call here; `kind` picks the message shown, never the server's text. */
export class ProjectApiError extends Error {
  constructor(
    readonly kind: ProjectFailureKind,
    readonly status: number | null
  ) {
    super(`Project request failed (${status ?? "network"}): ${kind}`);
    this.name = "ProjectApiError";
  }
}

/**
 * Checks a project name with Rust's rules (those of meeting titles) before any request.
 *
 * Outputs: the message to show, or `null` when the name is valid. Side effects: None.
 */
export function projectNameError(rawName: string): string | null {
  const { trimmedTitle, error } = validateMeetingTitle(rawName);
  if (error === null) return null;
  if (trimmedTitle.length === 0) return "Enter a name.";
  if (Array.from(trimmedTitle).length > maxMeetingTitleCharacters) {
    return `Names can be at most ${maxMeetingTitleCharacters} characters.`;
  }
  return "Names can't contain line breaks or control characters.";
}

/** A plain message for each failure kind, safe to show. */
export function projectErrorMessage(error: unknown): string {
  if (!(error instanceof ProjectApiError)) return "Something went wrong. Try again.";
  switch (error.kind) {
    case "invalid_name":
      return "That name isn't valid: use 1–200 characters on one line.";
    case "project_not_found":
      return "This project no longer exists.";
    case "meeting_not_found":
      return "That meeting no longer exists.";
    case "resource_not_found":
      return "That file is gone, or hasn't synced to this device yet.";
    case "rejected":
      return "That file type can't be added, or the file is larger than 100 MB.";
    case "not_an_email":
      return "That file isn't an e-mail.";
    case "unavailable":
      return "This server doesn't store project files.";
    case "network":
      return "The Koalino server can't be reached.";
    default:
      return "Something went wrong. Try again.";
  }
}

/**
 * Sends one request and decodes the JSON answer.
 *
 * Inputs: the path under the API, and the fetch options. Outputs: the parsed body, or
 * `undefined` for `204`. Throws `ProjectApiError` with the daemon's `error_code` (or `unavailable`
 * for 503), `unexpected`, or `network`. Side effects: an HTTP request to the daemon.
 */
async function projectRequest<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, { cache: "no-store", ...init });
  } catch {
    throw new ProjectApiError("network", null);
  }
  if (!response.ok) {
    const body = (await response.json().catch(() => ({}))) as { error_code?: unknown };
    const code = typeof body.error_code === "string" ? (body.error_code as ProjectFailureKind) : null;
    const kind = code !== null && knownKinds.includes(code) ? code : response.status === 503 ? "unavailable" : "unexpected";
    throw new ProjectApiError(kind, response.status);
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

/** Sends JSON with `method`. */
function sendJson<T>(path: string, method: string, body: unknown): Promise<T> {
  return projectRequest<T>(path, { method, headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
}

const projectPath = (projectId: string) => `/api/projects/${encodeURIComponent(projectId)}`;
const resourcePath = (projectId: string, attachmentId: string) =>
  `${projectPath(projectId)}/resources/${encodeURIComponent(attachmentId)}`;

/** Lists projects by name. */
export const getProjects = () => projectRequest<ProjectSummary[]>("/api/projects");

/** Reads a project with its meetings and resources. */
export const getProject = (projectId: string) => projectRequest<ProjectDetail>(projectPath(projectId));

/** Creates a project. Side effects: writes SQLite and syncs to paired devices. */
export const createProject = (name: string) => sendJson<ProjectSummary>("/api/projects", "POST", { name });

/** Renames a project. Side effects: as `createProject`. */
export const renameProject = (projectId: string, name: string) => sendJson<ProjectSummary>(projectPath(projectId), "PATCH", { name });

/** Deletes a project and its resources; its meetings stay. Side effects: reaches paired devices. */
export const deleteProject = (projectId: string) => projectRequest<void>(projectPath(projectId), { method: "DELETE" });

/** Moves a meeting into `projectId`, or out of any project with `null`. */
export const moveMeetingToProject = (meetingId: string, projectId: string | null) =>
  sendJson<void>(`/api/meetings/${encodeURIComponent(meetingId)}/project`, "PUT", { project_id: projectId });

/**
 * Uploads one file into a project; an `.eml` e-mail also adds its attachments.
 *
 * Inputs: the project and a browser `File` (up to 100 MiB). Side effects: the daemon stores it
 * and syncs it to paired devices.
 */
export const uploadProjectFile = (projectId: string, file: File) =>
  projectRequest<AddedResources>(`${projectPath(projectId)}/resources?name=${encodeURIComponent(file.name)}`, {
    method: "POST",
    headers: { "content-type": "application/octet-stream" },
    body: file
  });

/** Writes a typed note as a Markdown file in the project. */
export const addProjectNote = (projectId: string, title: string, text: string) =>
  sendJson<ProjectResource>(`${projectPath(projectId)}/notes`, "POST", { title, text });

/** Deletes one resource on every paired device. */
export const deleteProjectResource = (projectId: string, attachmentId: string) =>
  projectRequest<void>(resourcePath(projectId, attachmentId), { method: "DELETE" });

/** Reads an e-mail resource. */
export const getProjectEmail = (projectId: string, attachmentId: string) =>
  projectRequest<EmailView>(`${resourcePath(projectId, attachmentId)}/email`);

/** The URL serving a resource's bytes (sandboxed, never sniffed), for images, links and downloads. */
export const projectResourceFileUrl = (projectId: string, attachmentId: string) =>
  `${apiBaseUrl}${resourcePath(projectId, attachmentId)}/file`;

/**
 * Reads a text or Markdown resource as text, shown as-is (never rendered as HTML).
 *
 * Throws `ProjectApiError` like the other calls.
 */
export async function getProjectResourceText(projectId: string, attachmentId: string): Promise<string> {
  let response: Response;
  try {
    response = await fetch(projectResourceFileUrl(projectId, attachmentId), { cache: "no-store" });
  } catch {
    throw new ProjectApiError("network", null);
  }
  if (!response.ok) throw new ProjectApiError(response.status === 404 ? "resource_not_found" : "unexpected", response.status);
  return response.text();
}
