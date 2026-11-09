// One project's page (Issue #85): its meetings, files, notes and e-mails. Files are uploaded,
// notes written, meetings added or removed, and each resource previewed in the page: pictures
// inline, text and Markdown as plain text, e-mails as headers and text. PDFs and other documents
// open from the daemon, which serves them sandboxed. A search box finds words in this project's
// files and meetings only (#100).

"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { ChangeEvent, FormEvent, useState } from "react";
import { ConfirmDeleteDialog } from "@/components/confirm-delete-dialog";
import { SearchPanel } from "@/components/search-panel";
import type { MeetingSummary } from "@/lib/api";
import {
  addProjectNote,
  deleteProject,
  deleteProjectResource,
  getProject,
  getProjectEmail,
  getProjectResourceText,
  moveMeetingToProject,
  projectErrorMessage,
  projectNameError,
  projectResourceFileUrl,
  renameProject,
  uploadProjectFile,
  type EmailPerson,
  type EmailView,
  type ProjectDetail,
  type ProjectResource
} from "@/lib/projects-api";

const inputClass =
  "mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30";
const primaryButtonClass = "min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled";
const secondaryButtonClass = "min-h-11 rounded-lg border border-line-strong px-4 hover:bg-surface-hover disabled:text-disabled";

/** "12 KB", "3.4 MB", or "Syncing…" for a file not on the daemon yet. */
export function describeSize(sizeBytes: number | null): string {
  if (sizeBytes === null) return "Syncing…";
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${Math.round(sizeBytes / 1024)} KB`;
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** "Anna <anna@example.com>", or whichever part is known. */
function describePerson(person: EmailPerson): string {
  if (person.name && person.address) return `${person.name} <${person.address}>`;
  return person.name ?? person.address ?? "";
}

/** What a resource opens as in the page. */
type Preview =
  | { kind: "loading"; attachmentId: string }
  | { kind: "text"; attachmentId: string; text: string }
  | { kind: "email"; attachmentId: string; email: EmailView }
  | { kind: "image"; attachmentId: string }
  | { kind: "failed"; attachmentId: string; message: string };

/**
 * Shows and changes one project.
 *
 * Inputs: the project as the server rendered it, and every meeting (to offer those outside any
 * project). Outputs: the page's sections. Side effects: calls the project routes and reloads the
 * project after each change; deleting the project returns to `/projects`.
 */
export function ProjectView({ initialDetail, allMeetings }: { initialDetail: ProjectDetail; allMeetings: MeetingSummary[] }) {
  const router = useRouter();
  const [detail, setDetail] = useState(initialDetail);
  const [meetings, setMeetings] = useState(allMeetings);
  const [statusMessage, setStatusMessage] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [renameValue, setRenameValue] = useState<string | null>(null);
  const [isWritingNote, setIsWritingNote] = useState(false);
  const [noteTitle, setNoteTitle] = useState("");
  const [noteText, setNoteText] = useState("");
  const [meetingToAdd, setMeetingToAdd] = useState("");
  const [preview, setPreview] = useState<Preview | null>(null);
  const [pendingDeletion, setPendingDeletion] = useState<{ kind: "project" } | { kind: "resource"; resource: ProjectResource } | null>(null);
  const projectId = detail.project.project_id;
  const meetingsOutside = meetings.filter((meeting) => !meeting.project_id);

  /** Runs one change, then reloads the project; shows `success` or the failure's message. */
  async function change(action: () => Promise<unknown>, success: string | ((result: unknown) => string)) {
    setIsBusy(true);
    setErrorMessage(null);
    setStatusMessage("");
    try {
      const result = await action();
      setDetail(await getProject(projectId));
      setStatusMessage(typeof success === "string" ? success : success(result));
    } catch (error) {
      setErrorMessage(projectErrorMessage(error));
    } finally {
      setIsBusy(false);
    }
  }

  /** Uploads the picked files one after another. */
  async function upload(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length === 0) return;
    await change(async () => {
      let added = 0;
      let skipped = 0;
      for (const file of files) {
        const result = await uploadProjectFile(projectId, file);
        added += result.added.length;
        skipped += result.skipped_attachments;
      }
      return { added, skipped };
    }, (result) => {
      const { added, skipped } = result as { added: number; skipped: number };
      const message = `Added ${added} ${added === 1 ? "file" : "files"}`;
      return skipped > 0 ? `${message}; ${skipped} e-mail attachments were left in the e-mail` : message;
    });
  }

  /** Saves the typed note as a Markdown file. */
  async function saveNote(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const titleError = projectNameError(noteTitle);
    if (titleError !== null) {
      setErrorMessage(titleError.replace("name", "title").replace("Names", "Titles"));
      return;
    }
    await change(() => addProjectNote(projectId, noteTitle.trim(), noteText), "Note saved");
    setIsWritingNote(false);
    setNoteTitle("");
    setNoteText("");
  }

  /** Renames the project from the inline field. */
  async function saveName(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = renameValue ?? "";
    const nameError = projectNameError(name);
    if (nameError !== null) {
      setErrorMessage(nameError);
      return;
    }
    await change(() => renameProject(projectId, name.trim()), "Project renamed");
    setRenameValue(null);
    router.refresh();
  }

  /** Moves a meeting into this project, or out of it with `null`. */
  async function moveMeeting(meetingId: string, target: string | null) {
    await change(() => moveMeetingToProject(meetingId, target), target === null ? "Meeting removed from the project" : "Meeting added");
    setMeetings((current) => current.map((meeting) => (meeting.meeting_id === meetingId ? { ...meeting, project_id: target } : meeting)));
    setMeetingToAdd("");
  }

  /** Opens a resource in the page, or closes it when it is already open. */
  async function togglePreview(resource: ProjectResource) {
    if (preview?.attachmentId === resource.attachment_id) {
      setPreview(null);
      return;
    }
    const attachmentId = resource.attachment_id;
    if (resource.kind === "image") {
      setPreview({ kind: "image", attachmentId });
      return;
    }
    setPreview({ kind: "loading", attachmentId });
    try {
      if (resource.kind === "email") {
        setPreview({ kind: "email", attachmentId, email: await getProjectEmail(projectId, attachmentId) });
      } else {
        setPreview({ kind: "text", attachmentId, text: await getProjectResourceText(projectId, attachmentId) });
      }
    } catch (error) {
      setPreview({ kind: "failed", attachmentId, message: projectErrorMessage(error) });
    }
  }

  return (
    <div className="space-y-8">
      <header className="space-y-2">
        {renameValue === null ? (
          <div className="flex flex-wrap items-start gap-2">
            <h1 className="min-w-0 flex-1 text-2xl font-bold break-words sm:text-3xl">{detail.project.name}</h1>
            <button type="button" className={secondaryButtonClass} onClick={() => setRenameValue(detail.project.name)}>
              Rename
            </button>
            <button type="button" className={`${secondaryButtonClass} text-danger`} onClick={() => setPendingDeletion({ kind: "project" })}>
              Delete
            </button>
          </div>
        ) : (
          <form onSubmit={saveName} className="flex flex-col gap-2 sm:flex-row sm:items-end" noValidate>
            <label className="block min-w-0 flex-1 text-sm font-medium">
              Project name
              <input autoFocus value={renameValue} onChange={(event) => setRenameValue(event.target.value)} className={inputClass} />
            </label>
            <button type="submit" disabled={isBusy} className={primaryButtonClass}>
              Save
            </button>
            <button type="button" className={secondaryButtonClass} onClick={() => setRenameValue(null)}>
              Cancel
            </button>
          </form>
        )}
        <p role="status" aria-live="polite" className="text-sm text-success">
          {statusMessage}
        </p>
        <p role="alert" className="text-sm text-danger">
          {errorMessage}
        </p>
      </header>

      <SearchPanel scope="project" projectId={projectId} label="Search this project" />

      <section aria-labelledby="project-meetings" className="space-y-3">
        <h2 id="project-meetings" className="text-xl font-semibold">
          Meetings
        </h2>
        {detail.meetings.length === 0 ? <p className="text-muted">No meetings in this project yet.</p> : null}
        <ul className="divide-y divide-line rounded-xl border border-line bg-surface empty:hidden">
          {detail.meetings.map((meeting) => (
            <li key={meeting.meeting_id} className="flex items-center gap-2 px-4 py-2">
              <Link href={`/meetings/${encodeURIComponent(meeting.meeting_id)}`} className="min-w-0 flex-1 break-words py-1 font-medium hover:underline">
                {meeting.title}
              </Link>
              <button type="button" disabled={isBusy} className={secondaryButtonClass} onClick={() => moveMeeting(meeting.meeting_id, null)} aria-label={`Remove ${meeting.title} from the project`}>
                Remove
              </button>
            </li>
          ))}
        </ul>
        {meetingsOutside.length > 0 ? (
          <div className="flex flex-col gap-2 sm:flex-row sm:items-end">
            <label className="block min-w-0 flex-1 text-sm font-medium">
              Add a meeting
              <select value={meetingToAdd} onChange={(event) => setMeetingToAdd(event.target.value)} className={inputClass}>
                <option value="">Choose a meeting…</option>
                {meetingsOutside.map((meeting) => (
                  <option key={meeting.meeting_id} value={meeting.meeting_id}>
                    {meeting.title}
                  </option>
                ))}
              </select>
            </label>
            <button type="button" disabled={isBusy || meetingToAdd === ""} className={primaryButtonClass} onClick={() => moveMeeting(meetingToAdd, projectId)}>
              Add
            </button>
          </div>
        ) : null}
      </section>

      <section aria-labelledby="project-files" className="space-y-3">
        <h2 id="project-files" className="text-xl font-semibold">
          Files, notes and e-mails
        </h2>
        <div className="flex flex-wrap gap-2">
          <label className={`${primaryButtonClass} inline-flex cursor-pointer items-center focus-within:outline-2 focus-within:outline-focus`}>
            Add files
            <input type="file" multiple onChange={upload} disabled={isBusy} className="sr-only" />
          </label>
          <button type="button" className={secondaryButtonClass} onClick={() => setIsWritingNote((open) => !open)} aria-expanded={isWritingNote}>
            Write a note
          </button>
        </div>
        {isWritingNote ? (
          <form onSubmit={saveNote} className="space-y-2 rounded-xl border border-line bg-surface p-4" noValidate>
            <label className="block text-sm font-medium">
              Title
              <input value={noteTitle} onChange={(event) => setNoteTitle(event.target.value)} className={inputClass} />
            </label>
            <label className="block text-sm font-medium">
              Note (Markdown)
              <textarea value={noteText} onChange={(event) => setNoteText(event.target.value)} rows={6} className={`${inputClass} py-2`} />
            </label>
            <button type="submit" disabled={isBusy} className={primaryButtonClass}>
              Save note
            </button>
          </form>
        ) : null}
        {detail.resources.length === 0 ? <p className="text-muted">No files yet. Add PDFs, pictures, Markdown, or e-mails saved as .eml.</p> : null}
        <ul className="divide-y divide-line rounded-xl border border-line bg-surface empty:hidden">
          {detail.resources.map((resource) => (
            <li key={resource.attachment_id} id={`file-${resource.attachment_id}`} className="scroll-mt-20 space-y-2 px-4 py-3">
              <div className="flex flex-wrap items-center gap-2">
                <div className="min-w-0 flex-1">
                  <span className="block font-medium break-words">{resource.file_name}</span>
                  <span className="block text-sm text-muted">
                    {resource.kind} · {describeSize(resource.size_bytes)}
                  </span>
                </div>
                <ResourceActions
                  projectId={projectId}
                  resource={resource}
                  isOpen={preview?.attachmentId === resource.attachment_id}
                  onToggle={() => togglePreview(resource)}
                  onDelete={() => setPendingDeletion({ kind: "resource", resource })}
                />
              </div>
              {preview?.attachmentId === resource.attachment_id ? <ResourcePreview projectId={projectId} resource={resource} preview={preview} /> : null}
            </li>
          ))}
        </ul>
      </section>

      {pendingDeletion?.kind === "project" ? (
        <ConfirmDeleteDialog
          title={`Delete ${detail.project.name}?`}
          message="Its files, notes and e-mails are deleted on every paired device. Its meetings are kept."
          onClose={() => setPendingDeletion(null)}
          onConfirm={async () => {
            try {
              await deleteProject(projectId);
            } catch (error) {
              throw new Error(projectErrorMessage(error));
            }
            router.push("/projects");
            router.refresh();
          }}
        />
      ) : null}
      {pendingDeletion?.kind === "resource" ? (
        <ConfirmDeleteDialog
          title={`Delete ${pendingDeletion.resource.file_name}?`}
          message="It is deleted from this project on every paired device."
          onClose={() => setPendingDeletion(null)}
          onConfirm={async () => {
            try {
              await deleteProjectResource(projectId, pendingDeletion.resource.attachment_id);
            } catch (error) {
              throw new Error(projectErrorMessage(error));
            }
            setPendingDeletion(null);
            setPreview(null);
            setDetail(await getProject(projectId).catch(() => detail));
            setStatusMessage("File deleted");
          }}
        />
      ) : null}
    </div>
  );
}

/** View (in the page) or Open (from the daemon), Download, and Delete for one resource. */
function ResourceActions({
  projectId,
  resource,
  isOpen,
  onToggle,
  onDelete
}: {
  projectId: string;
  resource: ProjectResource;
  isOpen: boolean;
  onToggle: () => void;
  onDelete: () => void;
}) {
  const fileUrl = projectResourceFileUrl(projectId, resource.attachment_id);
  const isSynced = resource.size_bytes !== null;
  const opensInPage = resource.kind === "image" || resource.kind === "markdown" || resource.kind === "text" || resource.kind === "email";
  return (
    <div className="flex flex-wrap gap-2">
      {isSynced && opensInPage ? (
        <button type="button" className={secondaryButtonClass} onClick={onToggle} aria-expanded={isOpen} aria-label={`${isOpen ? "Close" : "View"} ${resource.file_name}`}>
          {isOpen ? "Close" : "View"}
        </button>
      ) : null}
      {isSynced && resource.kind === "pdf" ? (
        <a href={fileUrl} target="_blank" rel="noopener noreferrer" className={`${secondaryButtonClass} inline-flex items-center`} aria-label={`Open ${resource.file_name}`}>
          Open
        </a>
      ) : null}
      {isSynced ? (
        <a href={fileUrl} download={resource.file_name} className={`${secondaryButtonClass} inline-flex items-center`} aria-label={`Download ${resource.file_name}`}>
          Download
        </a>
      ) : null}
      <button type="button" className={`${secondaryButtonClass} text-danger`} onClick={onDelete} aria-label={`Delete ${resource.file_name}`}>
        Delete
      </button>
    </div>
  );
}

/** The open resource: a picture, plain text (Markdown is not rendered), or an e-mail. */
function ResourcePreview({ projectId, resource, preview }: { projectId: string; resource: ProjectResource; preview: Preview }) {
  switch (preview.kind) {
    case "loading":
      return <p className="text-muted">Loading…</p>;
    case "failed":
      return <p className="text-danger">{preview.message}</p>;
    case "image":
      return (
        // eslint-disable-next-line @next/next/no-img-element -- served by the local daemon, not optimisable.
        <img src={projectResourceFileUrl(projectId, resource.attachment_id)} alt={resource.file_name} className="max-h-[70vh] max-w-full rounded-lg" />
      );
    case "text":
      return <pre className="max-h-[70vh] overflow-auto rounded-lg bg-surface-muted p-3 text-sm whitespace-pre-wrap break-words">{preview.text}</pre>;
    case "email":
      return <EmailPreview email={preview.email} />;
  }
}

/** An e-mail's headers, its text, and the attachments that came with it. */
function EmailPreview({ email }: { email: EmailView }) {
  const rows: [string, string][] = [
    ["From", email.from.map(describePerson).join(", ")],
    ["To", email.to.map(describePerson).join(", ")],
    ["Cc", email.cc.map(describePerson).join(", ")],
    ["Date", email.sent_at ? new Date(email.sent_at).toLocaleString() : ""]
  ];
  return (
    <article className="space-y-2 rounded-lg bg-surface-muted p-3">
      <h3 className="font-semibold break-words">{email.subject ?? "(no subject)"}</h3>
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 text-sm">
        {rows
          .filter(([, value]) => value !== "")
          .map(([label, value]) => (
            <div key={label} className="contents">
              <dt className="text-muted">{label}</dt>
              <dd className="break-words">{value}</dd>
            </div>
          ))}
      </dl>
      <pre className="max-h-[60vh] overflow-auto text-sm whitespace-pre-wrap break-words">{email.body_text}</pre>
      {email.body_truncated ? <p className="text-sm text-muted">The text was shortened.</p> : null}
      {email.attachments.length > 0 ? (
        <p className="text-sm text-muted">Attachments: {email.attachments.map((attachment) => attachment.file_name).join(", ")}</p>
      ) : null}
    </article>
  );
}
