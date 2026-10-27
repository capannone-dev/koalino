// Renders one meeting's title, transcript, and metadata-only attachment list from Axum.

import { getMeeting } from "@/lib/api";

export default async function MeetingDetailPage({ params }: { params: Promise<{ meetingId: string }> }) {
  const { meetingId } = await params;
  const meeting = await getMeeting(meetingId);
  return (
    <section>
      <h1>{meeting.title}</h1>
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
