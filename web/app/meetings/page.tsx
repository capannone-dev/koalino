// Renders the meeting index (title, then ID) from the Axum meeting metadata endpoint.

import Link from "next/link";
import { getMeetings } from "@/lib/api";

export default async function MeetingsPage() {
  const meetings = await getMeetings();
  return (
    <section>
      <h1>Meetings</h1>
      <ul>
        {meetings.map((meeting) => (
          <li key={meeting.meeting_id}>
            <Link href={`/meetings/${encodeURIComponent(meeting.meeting_id)}`}>{meeting.title}</Link>{" "}
            <small>{meeting.meeting_id}</small>
          </li>
        ))}
      </ul>
    </section>
  );
}
