// Renders the web Devices page (Issue #43): device sync between the owner's own devices, read on
// the server first and then kept current by the client panel.

import { DevicesPanel } from "@/components/devices-panel";
import { getP2pStatus } from "@/lib/p2p-api";

export default async function DevicesPage() {
  const status = await getP2pStatus().catch(() => null);
  return (
    <section className="space-y-4">
      <h1 className="text-2xl font-bold sm:text-3xl">Devices</h1>
      <p className="max-w-2xl text-muted">Pair your own devices to keep their meetings in sync over the local network.</p>
      <DevicesPanel initialStatus={status} />
    </section>
  );
}
