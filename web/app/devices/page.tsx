// Renders the web Devices page (Issue #43): device sync between the owner's own devices, read on
// the server first and then kept current by the client panel.

import { DevicesPanel } from "@/components/devices-panel";
import { getP2pStatus } from "@/lib/p2p-api";
import { getT } from "@/lib/i18n/server";

export default async function DevicesPage() {
  const t = await getT();
  const status = await getP2pStatus().catch(() => null);
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">{t("devices")}</h1>
      <p className="max-w-2xl text-muted">{t("pairYourOwnDevicesTo")}</p>
      <DevicesPanel initialStatus={status} />
    </section>
  );
}
