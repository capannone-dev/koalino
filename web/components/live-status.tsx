// Connects a browser to the Axum WebSocket and renders only safe status-event text.

"use client";

import { useEffect, useState } from "react";
import { webSocketUrl } from "@/lib/api";
import { useT } from "@/lib/i18n/client";

export function LiveStatus() {
  const t = useT();
  const [status, setStatus] = useState(t("connectingToLiveUpdates"));
  useEffect(() => {
    const socket = new WebSocket(webSocketUrl());
    socket.onopen = () => setStatus(t("connectedForLiveUpdates"));
    socket.onmessage = (event) => setStatus(t("update", { data: event.data }));
    socket.onerror = () => setStatus(t("liveUpdatesAreUnavailable"));
    return () => socket.close();
  }, [t]);
  return <p aria-live="polite">{status}</p>;
}
