// Renders the meeting index from the Axum meeting metadata endpoint; rows can be renamed in place.
// A search box above finds words in every meeting's transcript, summary and notes (#100).

import { MeetingList } from "@/components/meeting-list";
import { SearchPanel } from "@/components/search-panel";
import { getMeetings } from "@/lib/api";

export default async function MeetingsPage() {
  const meetings = await getMeetings();
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">Meetings</h1>
      <SearchPanel scope="meetings" label="Search meetings" />
      <MeetingList meetings={meetings} />
    </section>
  );
}
