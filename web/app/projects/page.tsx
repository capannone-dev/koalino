// Renders the project index (Issue #85) from `GET /api/projects`; projects are created in place.
// A search box finds words in every project's files and meetings (#100).

import { ProjectsIndex } from "@/components/projects-index";
import { SearchPanel } from "@/components/search-panel";
import { getProjects } from "@/lib/projects-api";

/** Always rendered per request: projects change on other devices, and builds run without a daemon. */
export const dynamic = "force-dynamic";

export default async function ProjectsPage() {
  const projects = await getProjects();
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">Projects</h1>
      <SearchPanel scope="projects" label="Search all projects" />
      <ProjectsIndex projects={projects} />
    </section>
  );
}
