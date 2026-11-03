// Renders the project index (Issue #85) from `GET /api/projects`; projects are created in place.

import { ProjectsIndex } from "@/components/projects-index";
import { getProjects } from "@/lib/projects-api";

/** Always rendered per request: projects change on other devices, and builds run without a daemon. */
export const dynamic = "force-dynamic";

export default async function ProjectsPage() {
  const projects = await getProjects();
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">Projects</h1>
      <ProjectsIndex projects={projects} />
    </section>
  );
}
