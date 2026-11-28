// The web's home (#205): it opens the meetings, which are what people come for. The live-updates
// status moved to the header (`LiveStatus`).

import { redirect } from "next/navigation";

/** Sends the visitor to the meeting list. Side effects: a redirect. */
export default function HomePage() {
  redirect("/meetings");
}
