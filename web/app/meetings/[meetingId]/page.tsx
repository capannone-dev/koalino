// Renders one meeting's renamable title, its project (Issue #85), transcript, and metadata-only
// attachment list from Axum.

import { MeetingTitleHeader } from "@/components/meeting-title-header";
import { MeetingProjectPicker } from "@/components/meeting-project-picker";
import { getMeeting, getMeetings } from "@/lib/api";
import { getProjects, type ProjectSummary } from "@/lib/projects-api";

export default async function MeetingDetailPage({ params }: { params: Promise<{ meetingId: string }> }) {
  const { meetingId } = await params;
  const meeting = await getMeeting(meetingId);
  // The project picker is extra: an older daemon without projects still shows the meeting.
  const [projects, summaries] = await Promise.all([
    getProjects().catch((): ProjectSummary[] | null => null),
    getMeetings().catch(() => [])
  ]);
  const projectId = summaries.find((summary) => summary.meeting_id === meetingId)?.project_id ?? null;
  return (
    <section>
      <MeetingTitleHeader
        meeting={{ meeting_id: meeting.meeting_id, title: meeting.title, created_at: meeting.created_at }}
      />
      {projects === null ? null : <MeetingProjectPicker meetingId={meeting.meeting_id} projectId={projectId} projects={projects} />}
      <p>Meeting {meeting.meeting_id}</p>
      <h2>Transcript</h2>
      <p>{meeting.transcript ?? "No transcript is available."}</p>
      <h2>Attachments</h2>
      <ul>
        {meeting.attachments.map((attachment) => (
          <li key={attachment.attachment_id}>{attachment.file_path} ({attachment.mime_type})</li>
        ))}
      </ul>
    </section>
  );
}
