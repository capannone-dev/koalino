// Lists projects (Issue #85) with their meeting and file counts, subprojects indented under their
// parent (#142), and creates new ones.

"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { FormEvent, useState } from "react";
import { createProject, projectErrorMessage, projectNameError, projectTree, type ProjectSummary } from "@/lib/projects-api";
import { useT } from "@/lib/i18n/client";

/** "2 meetings · 3 files", singular where it applies. */
export function projectCounts(project: ProjectSummary): string {
  const meetings = `${project.meeting_count} ${project.meeting_count === 1 ? "meeting" : "meetings"}`;
  const files = `${project.resource_count} ${project.resource_count === 1 ? "file" : "files"}`;
  return `${meetings} · ${files}`;
}

/**
 * Shows the projects as links, and a form that creates one and opens it.
 *
 * Inputs: the projects as the server rendered them. Outputs: the list and the form. Side
 * effects: `POST /api/projects`, then navigates to the new project.
 */
export function ProjectsIndex({ projects }: { projects: ProjectSummary[] }) {
  const t = useT();
  const router = useRouter();
  const [name, setName] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  /** Validates the name as Rust does, creates the project and opens it. */
  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nameError = projectNameError(name, t);
    if (nameError !== null) {
      setErrorMessage(nameError);
      return;
    }
    setIsCreating(true);
    setErrorMessage(null);
    try {
      const created = await createProject(name.trim());
      router.push(`/projects/${encodeURIComponent(created.project_id)}`);
    } catch (error) {
      setErrorMessage(projectErrorMessage(error, t));
      setIsCreating(false);
    }
  }

  return (
    <div className="space-y-6">
      <form onSubmit={create} className="flex flex-col gap-2 sm:flex-row sm:items-end" noValidate>
        <label className="block min-w-0 flex-1 text-sm font-medium">{t("newProject2")}<input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={t("projectName")}
            aria-invalid={errorMessage !== null}
            className="mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30"
          />
        </label>
        <button
          type="submit"
          disabled={isCreating}
          className="min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled"
        >{t("create")}</button>
      </form>
      <p role="alert" className="text-sm text-danger">
        {errorMessage}
      </p>
      {projects.length === 0 ? (
        <p className="text-muted">{t("noProjectsYetAProject")}</p>
      ) : (
        <ul className="divide-y divide-line rounded-xl border border-line bg-surface">
          {projectTree(projects).map(({ project, depth }) => (
            <li key={project.project_id}>
              <Link
                href={`/projects/${encodeURIComponent(project.project_id)}`}
                data-depth={depth}
                style={{ paddingInlineStart: `${1 + depth * 1.5}rem` }}
                className="block px-4 py-3 hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-focus"
              >
                <span className="block font-medium break-words">{project.name}</span>
                <span className="block text-sm text-muted">{projectCounts(project)}</span>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
