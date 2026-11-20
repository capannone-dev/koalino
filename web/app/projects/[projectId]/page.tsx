// Renders one project (Issue #85) with every meeting, so meetings outside projects can be added,
// and every project, for its subprojects and Move to (#142).

import { ProjectView } from "@/components/project-view";
import { getMeetings } from "@/lib/api";
import { getProject, getProjects } from "@/lib/projects-api";

/** Always rendered per request: projects change on other devices, and builds run without a daemon. */
export const dynamic = "force-dynamic";

export default async function ProjectPage({ params }: { params: Promise<{ projectId: string }> }) {
  const { projectId } = await params;
  const [detail, meetings, projects] = await Promise.all([getProject(projectId), getMeetings(), getProjects()]);
  return (
    <section>
      <ProjectView key={detail.project.project_id} initialDetail={detail} allMeetings={meetings} allProjects={projects} />
    </section>
  );
}
