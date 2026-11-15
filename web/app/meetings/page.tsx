// Renders the meeting index from the Axum meeting metadata endpoint; rows can be renamed in place.
// A search box above finds words in every meeting's transcript, summary and notes (#100).

import { MeetingList } from "@/components/meeting-list";
import { SearchPanel } from "@/components/search-panel";
import { getMeetings } from "@/lib/api";
import { getT } from "@/lib/i18n/server";

export default async function MeetingsPage() {
  const t = await getT();
  const meetings = await getMeetings();
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">{t("meetings")}</h1>
      <SearchPanel scope="meetings" label={t("searchMeetings")} />
      <MeetingList meetings={meetings} />
    </section>
  );
}
