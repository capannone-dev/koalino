// Connects a browser to the Axum WebSocket and renders only safe status-event text.

"use client";

import { useEffect, useState } from "react";
import { webSocketUrl } from "@/lib/api";

export function LiveStatus() {
  const [status, setStatus] = useState("Connecting to live updates…");
  useEffect(() => {
    const socket = new WebSocket(webSocketUrl());
    socket.onopen = () => setStatus("Connected for live updates.");
    socket.onmessage = (event) => setStatus(`Update: ${event.data}`);
    socket.onerror = () => setStatus("Live updates are unavailable.");
    return () => socket.close();
  }, []);
  return <p aria-live="polite">{status}</p>;
}
