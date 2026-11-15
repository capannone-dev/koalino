// Renders the server home: live Axum WebSocket status. Telemetry is under Settings (Issue #53).

import { LiveStatus } from "@/components/live-status";
import { getT } from "@/lib/i18n/server";

export default async function HomePage() {
  const t = await getT();
  return (
    <section className="space-y-2">
      <h1 className="text-2xl font-bold sm:text-3xl">{t("koalinoServer")}</h1>
      <LiveStatus />
    </section>
  );
}
