// Renders the Chat page (#97): questions about every meeting, summary and project, answered by
// the default AI connection with the notes that fit.

import { ChatPanel } from "@/components/chat-panel";
import { getT } from "@/lib/i18n/server";

export default async function ChatPage() {
  const t = await getT();
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">{t("chat")}</h1>
      <ChatPanel topic={{ scope: "all" }} heading={t("chatWithAllYourNotes")} />
    </section>
  );
}
