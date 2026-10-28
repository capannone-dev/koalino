// Sends old `/dashboard` bookmarks to the telemetry dashboard's new home under Settings (Issue #53).

import { redirect } from "next/navigation";

export default function DashboardRedirect() {
  redirect("/settings/telemetry");
}
