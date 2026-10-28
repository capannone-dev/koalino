// Renders one meeting's renamable title, transcript, and metadata-only attachment list from Axum.

import { MeetingTitleHeader } from "@/components/meeting-title-header";
import { getMeeting } from "@/lib/api";

export default async function MeetingDetailPage({ params }: { params: Promise<{ meetingId: string }> }) {
  const { meetingId } = await params;
  const meeting = await getMeeting(meetingId);
  return (
    <section>
      <MeetingTitleHeader
        meeting={{ meeting_id: meeting.meeting_id, title: meeting.title, created_at: meeting.created_at }}
      />
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
