// Renders the meeting index from the Axum meeting metadata endpoint; rows can be renamed in place.

import { MeetingList } from "@/components/meeting-list";
import { getMeetings } from "@/lib/api";

export default async function MeetingsPage() {
  const meetings = await getMeetings();
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">Meetings</h1>
      <MeetingList meetings={meetings} />
    </section>
  );
}
