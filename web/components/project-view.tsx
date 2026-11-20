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
import { ChatPanel } from "@/components/chat-panel";
import { SearchPanel } from "@/components/search-panel";
import type { MeetingSummary } from "@/lib/api";
import {
  addProjectNote,
  createProject,
  deleteProject,
  deleteProjectResource,
  getProject,
  getProjectEmail,
  getProjectResourceText,
  isWithinProject,
  moveMeetingToProject,
  moveProject,
  projectErrorMessage,
  projectNameError,
  projectResourceFileUrl,
  projectTree,
  renameProject,
  uploadProjectFile,
  type EmailPerson,
  type EmailView,
  type ProjectDetail,
  type ProjectResource,
  type ProjectSummary
} from "@/lib/projects-api";
import { projectCounts } from "@/components/projects-index";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";

const inputClass =
  "mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30";
const primaryButtonClass = "min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled";
const secondaryButtonClass = "min-h-11 rounded-lg border border-line-strong px-4 hover:bg-surface-hover disabled:text-disabled";

/** "12 KB", "3.4 MB", or "Syncing…" for a file not on the daemon yet. */
export function describeSize(t: Translate, sizeBytes: number | null): string {
  if (sizeBytes === null) return t("syncing");
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
 * Inputs: the project as the server rendered it, every meeting (to offer those outside any
 * project), and every project (for its subprojects and Move to, #142). Outputs: the page's
 * sections. Side effects: calls the project routes and reloads the project after each change;
 * deleting the project returns to `/projects`; creating a subproject opens it.
 */
export function ProjectView({
  initialDetail,
  allMeetings,
  allProjects = []
}: {
  initialDetail: ProjectDetail;
  allMeetings: MeetingSummary[];
  allProjects?: ProjectSummary[];
}) {
  const t = useT();
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
  const [projects, setProjects] = useState(allProjects);
  const currentParent = projects.find((project) => project.project_id === projectId)?.parent_id ?? "";
  const [moveTarget, setMoveTarget] = useState<string>(currentParent);
  const [subprojectName, setSubprojectName] = useState("");
  const subprojects = projectTree(projects)
    .filter(({ project }) => project.parent_id === projectId)
    .map(({ project }) => project);
  const moveTargets = projectTree(projects).filter(({ project }) => !isWithinProject(projects, project.project_id, projectId));
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
      setErrorMessage(projectErrorMessage(error, t));
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
      const message = t("addedFiles", { count: added });
      return skipped > 0 ? t("eMailAttachmentsWereLeft", { message, skipped }) : message;
    });
  }

  /** Saves the typed note as a Markdown file. */
  async function saveNote(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const titleError = projectNameError(noteTitle, t);
    if (titleError !== null) {
      setErrorMessage(titleError.replace("name", "title").replace("Names", "Titles"));
      return;
    }
    await change(() => addProjectNote(projectId, noteTitle.trim(), noteText), t("noteSaved"));
    setIsWritingNote(false);
    setNoteTitle("");
    setNoteText("");
  }

  /** Renames the project from the inline field. */
  async function saveName(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = renameValue ?? "";
    const nameError = projectNameError(name, t);
    if (nameError !== null) {
      setErrorMessage(nameError);
      return;
    }
    await change(() => renameProject(projectId, name.trim()), t("projectRenamed"));
    setRenameValue(null);
    router.refresh();
  }

  /** Creates a subproject here and opens it (#142). */
  async function createSubproject(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nameError = projectNameError(subprojectName, t);
    if (nameError !== null) {
      setErrorMessage(nameError);
      return;
    }
    setIsBusy(true);
    setErrorMessage(null);
    try {
      const created = await createProject(subprojectName.trim(), projectId);
      router.push(`/projects/${encodeURIComponent(created.project_id)}`);
    } catch (error) {
      setErrorMessage(projectErrorMessage(error, t));
      setIsBusy(false);
    }
  }

  /** Moves this project inside the chosen one, or to the top level (#142). */
  async function moveHere() {
    const parentId = moveTarget === "" ? null : moveTarget;
    await change(() => moveProject(projectId, parentId), t("projectMoved"));
    setProjects((current) => current.map((project) => (project.project_id === projectId ? { ...project, parent_id: parentId } : project)));
    router.refresh();
  }

  /** Moves a meeting into this project, or out of it with `null`. */
  async function moveMeeting(meetingId: string, target: string | null) {
    await change(() => moveMeetingToProject(meetingId, target), target === null ? t("meetingRemovedFromTheProject") : t("meetingAdded"));
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
      setPreview({ kind: "failed", attachmentId, message: projectErrorMessage(error, t) });
    }
  }

  return (
    <div className="space-y-8">
      <header className="space-y-2">
        {renameValue === null ? (
          <div className="flex flex-wrap items-start gap-2">
            <h1 className="min-w-0 flex-1 text-2xl font-bold break-words sm:text-3xl">{detail.project.name}</h1>
            <button type="button" className={secondaryButtonClass} onClick={() => setRenameValue(detail.project.name)}>{t("rename")}</button>
            <button type="button" className={`${secondaryButtonClass} text-danger`} onClick={() => setPendingDeletion({ kind: "project" })}>{t("delete")}</button>
          </div>
        ) : (
          <form onSubmit={saveName} className="flex flex-col gap-2 sm:flex-row sm:items-end" noValidate>
            <label className="block min-w-0 flex-1 text-sm font-medium">{t("projectName")}<input autoFocus value={renameValue} onChange={(event) => setRenameValue(event.target.value)} className={inputClass} />
            </label>
            <button type="submit" disabled={isBusy} className={primaryButtonClass}>{t("save")}</button>
            <button type="button" className={secondaryButtonClass} onClick={() => setRenameValue(null)}>{t("cancel")}</button>
          </form>
        )}
        <div className="flex flex-col gap-2 sm:flex-row sm:items-end">
          <label className="block min-w-0 flex-1 text-sm font-medium">{t("moveProjectTo")}<select value={moveTarget} onChange={(event) => setMoveTarget(event.target.value)} className={inputClass}>
              <option value="">{t("topLevel")}</option>
              {moveTargets.map(({ project, depth }) => (
                <option key={project.project_id} value={project.project_id}>
                  {`${"\u00a0\u00a0".repeat(depth)}${project.name}`}
                </option>
              ))}
            </select>
          </label>
          <button type="button" disabled={isBusy || moveTarget === currentParent} className={secondaryButtonClass} onClick={moveHere}>{t("moveProject")}</button>
        </div>
        <p role="status" aria-live="polite" className="text-sm text-success">
          {statusMessage}
        </p>
        <p role="alert" className="text-sm text-danger">
          {errorMessage}
        </p>
      </header>

      <SearchPanel scope="project" projectId={projectId} label={t("searchThisProject")} />
      <ChatPanel topic={{ scope: "project", id: projectId }} heading={t("chatAboutThisProject")} />

      <section aria-labelledby="project-subprojects" className="space-y-3">
        <h2 id="project-subprojects" className="text-xl font-semibold">{t("subprojects")}</h2>
        <ul className="divide-y divide-line rounded-xl border border-line bg-surface empty:hidden">
          {subprojects.map((project) => (
            <li key={project.project_id}>
              <Link href={`/projects/${encodeURIComponent(project.project_id)}`} className="block px-4 py-2 hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-focus">
                <span className="block font-medium break-words">{project.name}</span>
                <span className="block text-sm text-muted">{projectCounts(project)}</span>
              </Link>
            </li>
          ))}
        </ul>
        <form onSubmit={createSubproject} className="flex flex-col gap-2 sm:flex-row sm:items-end" noValidate>
          <label className="block min-w-0 flex-1 text-sm font-medium">{t("newSubproject")}<input value={subprojectName} onChange={(event) => setSubprojectName(event.target.value)} placeholder={t("projectName")} className={inputClass} />
          </label>
          <button type="submit" disabled={isBusy} className={primaryButtonClass}>{t("create")}</button>
        </form>
      </section>

      <section aria-labelledby="project-meetings" className="space-y-3">
        <h2 id="project-meetings" className="text-xl font-semibold">{t("meetings")}</h2>
        {detail.meetings.length === 0 ? <p className="text-muted">{t("noMeetingsInThisProject")}</p> : null}
        <ul className="divide-y divide-line rounded-xl border border-line bg-surface empty:hidden">
          {detail.meetings.map((meeting) => (
            <li key={meeting.meeting_id} className="flex items-center gap-2 px-4 py-2">
              <Link href={`/meetings/${encodeURIComponent(meeting.meeting_id)}`} className="min-w-0 flex-1 break-words py-1 font-medium hover:underline">
                {meeting.title}
              </Link>
              <button type="button" disabled={isBusy} className={secondaryButtonClass} onClick={() => moveMeeting(meeting.meeting_id, null)} aria-label={t("removeFromTheProject", { title: meeting.title })}>{t("remove")}</button>
            </li>
          ))}
        </ul>
        {meetingsOutside.length > 0 ? (
          <div className="flex flex-col gap-2 sm:flex-row sm:items-end">
            <label className="block min-w-0 flex-1 text-sm font-medium">{t("addAMeeting")}<select value={meetingToAdd} onChange={(event) => setMeetingToAdd(event.target.value)} className={inputClass}>
                <option value="">{t("chooseAMeeting")}</option>
                {meetingsOutside.map((meeting) => (
                  <option key={meeting.meeting_id} value={meeting.meeting_id}>
                    {meeting.title}
                  </option>
                ))}
              </select>
            </label>
            <button type="button" disabled={isBusy || meetingToAdd === ""} className={primaryButtonClass} onClick={() => moveMeeting(meetingToAdd, projectId)}>{t("add")}</button>
          </div>
        ) : null}
      </section>

      <section aria-labelledby="project-files" className="space-y-3">
        <h2 id="project-files" className="text-xl font-semibold">{t("filesNotesAndEMails")}</h2>
        <div className="flex flex-wrap gap-2">
          <label className={`${primaryButtonClass} inline-flex cursor-pointer items-center focus-within:outline-2 focus-within:outline-focus`}>{t("addFiles")}<input type="file" multiple onChange={upload} disabled={isBusy} className="sr-only" />
          </label>
          <button type="button" className={secondaryButtonClass} onClick={() => setIsWritingNote((open) => !open)} aria-expanded={isWritingNote}>{t("writeANote")}</button>
        </div>
        {isWritingNote ? (
          <form onSubmit={saveNote} className="space-y-2 rounded-xl border border-line bg-surface p-4" noValidate>
            <label className="block text-sm font-medium">{t("title")}<input value={noteTitle} onChange={(event) => setNoteTitle(event.target.value)} className={inputClass} />
            </label>
            <label className="block text-sm font-medium">{t("noteMarkdown")}<textarea value={noteText} onChange={(event) => setNoteText(event.target.value)} rows={6} className={`${inputClass} py-2`} />
            </label>
            <button type="submit" disabled={isBusy} className={primaryButtonClass}>{t("saveNote")}</button>
          </form>
        ) : null}
        {detail.resources.length === 0 ? <p className="text-muted">{t("noFilesYetAddPdfs")}</p> : null}
        <ul className="divide-y divide-line rounded-xl border border-line bg-surface empty:hidden">
          {detail.resources.map((resource) => (
            <li key={resource.attachment_id} id={`file-${resource.attachment_id}`} className="scroll-mt-20 space-y-2 px-4 py-3">
              <div className="flex flex-wrap items-center gap-2">
                <div className="min-w-0 flex-1">
                  <span className="block font-medium break-words">{resource.file_name}</span>
                  <span className="block text-sm text-muted">
                    {resource.kind} · {describeSize(t, resource.size_bytes)}
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
          title={t("delete3", { name: detail.project.name })}
          message={t("itsFilesNotesAndE")}
          onClose={() => setPendingDeletion(null)}
          onConfirm={async () => {
            try {
              await deleteProject(projectId);
            } catch (error) {
              throw new Error(projectErrorMessage(error, t));
            }
            router.push("/projects");
            router.refresh();
          }}
        />
      ) : null}
      {pendingDeletion?.kind === "resource" ? (
        <ConfirmDeleteDialog
          title={t("delete4", { fileName: pendingDeletion.resource.file_name })}
          message={t("itIsDeletedFromThis")}
          onClose={() => setPendingDeletion(null)}
          onConfirm={async () => {
            try {
              await deleteProjectResource(projectId, pendingDeletion.resource.attachment_id);
            } catch (error) {
              throw new Error(projectErrorMessage(error, t));
            }
            setPendingDeletion(null);
            setPreview(null);
            setDetail(await getProject(projectId).catch(() => detail));
            setStatusMessage(t("fileDeleted"));
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
  const t = useT();
  const fileUrl = projectResourceFileUrl(projectId, resource.attachment_id);
  const isSynced = resource.size_bytes !== null;
  const opensInPage = resource.kind === "image" || resource.kind === "markdown" || resource.kind === "text" || resource.kind === "email";
  return (
    <div className="flex flex-wrap gap-2">
      {isSynced && opensInPage ? (
        <button type="button" className={secondaryButtonClass} onClick={onToggle} aria-expanded={isOpen} aria-label={`${isOpen ? t("close") : t("view")} ${resource.file_name}`}>
          {isOpen ? t("close") : t("view")}
        </button>
      ) : null}
      {isSynced && resource.kind === "pdf" ? (
        <a href={fileUrl} target="_blank" rel="noopener noreferrer" className={`${secondaryButtonClass} inline-flex items-center`} aria-label={t("open", { fileName: resource.file_name })}>{t("open2")}</a>
      ) : null}
      {isSynced ? (
        <a href={fileUrl} download={resource.file_name} className={`${secondaryButtonClass} inline-flex items-center`} aria-label={t("download", { fileName: resource.file_name })}>{t("download2")}</a>
      ) : null}
      <button type="button" className={`${secondaryButtonClass} text-danger`} onClick={onDelete} aria-label={t("delete5", { fileName: resource.file_name })}>{t("delete")}</button>
    </div>
  );
}

/** The open resource: a picture, plain text (Markdown is not rendered), or an e-mail. */
function ResourcePreview({ projectId, resource, preview }: { projectId: string; resource: ProjectResource; preview: Preview }) {
  const t = useT();
  switch (preview.kind) {
    case "loading":
      return <p className="text-muted">{t("loading")}</p>;
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
  const t = useT();
  const rows: [string, string][] = [
    [t("from"), email.from.map(describePerson).join(", ")],
    [t("to"), email.to.map(describePerson).join(", ")],
    ["Cc", email.cc.map(describePerson).join(", ")],
    [t("date"), email.sent_at ? new Date(email.sent_at).toLocaleString() : ""]
  ];
  return (
    <article className="space-y-2 rounded-lg bg-surface-muted p-3">
      <h3 className="font-semibold break-words">{email.subject ?? t("noSubject")}</h3>
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
      {email.body_truncated ? <p className="text-sm text-muted">{t("theTextWasShortened")}</p> : null}
      {email.attachments.length > 0 ? (
        <p className="text-sm text-muted">{t("attachments2", { names: email.attachments.map((attachment) => attachment.file_name).join(", ") })}</p>
      ) : null}
    </article>
  );
}
