// Connects a browser to the Axum WebSocket and shows whether live updates arrive (#205): a small
// coloured dot in the header with its meaning in words for screen readers and as a tooltip. The
// events' own text is never shown.

"use client";

import { useEffect, useState } from "react";
import { webSocketUrl } from "@/lib/api";
import { useT } from "@/lib/i18n/client";

/** Where the live-updates connection stands. */
type LiveState = "connecting" | "connected" | "unavailable";

/** Shows the live-updates state. Side effects: opens a WebSocket to the daemon while mounted. */
export function LiveStatus() {
  const t = useT();
  const [state, setState] = useState<LiveState>("connecting");
  useEffect(() => {
    const socket = new WebSocket(webSocketUrl());
    socket.onopen = () => setState("connected");
    socket.onmessage = () => setState("connected");
    socket.onerror = () => setState("unavailable");
    return () => socket.close();
  }, []);
  const label = {
    connecting: t("connectingToLiveUpdates"),
    connected: t("connectedForLiveUpdates"),
    unavailable: t("liveUpdatesAreUnavailable")
  }[state];
  const colour = { connecting: "bg-faint", connected: "bg-success", unavailable: "bg-warning" }[state];
  return (
    <span role="status" aria-live="polite" title={label} className="inline-flex items-center">
      <span aria-hidden="true" className={`inline-block h-2.5 w-2.5 rounded-full ${colour}`} />
      <span className="sr-only">{label}</span>
    </span>
  );
}
