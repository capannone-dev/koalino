// Coordinates real-time telemetry refreshes and responsive Recharts visualizations.

"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { Bar, BarChart, Cell, Legend, Line, LineChart, Pie, PieChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import { type Device, type DeviceTelemetry, type TelemetrySummary, getDeviceTelemetry, getDevices, getTelemetrySummary, webSocketUrl } from "@/lib/api";
import { useT } from "@/lib/i18n/client";

export type DashboardData = { devices: Device[]; summary: TelemetrySummary; series: DeviceTelemetry[] };

/** Renders summary, device status, charts, and WebSocket-driven telemetry refreshes. */
export function TelemetryDashboard({ initialData }: { initialData: DashboardData }) {
  const t = useT();
  const [data, setData] = useState(initialData);
  const [isLive, setIsLive] = useState(false);
  const lastRefresh = useRef(0);
  const refresh = async () => {
    if (Date.now() - lastRefresh.current < 10_000) return;
    lastRefresh.current = Date.now();
    const devicesResponse = await getDevices();
    const series = await Promise.all(devicesResponse.devices.map(async (device) => getDeviceTelemetry(device.device_id).catch(() => ({ device_id: device.device_id, metrics: [] }))));
    setData({ devices: devicesResponse.devices, summary: await getTelemetrySummary(), series });
  };
  useEffect(() => {
    let socket: WebSocket | undefined;
    let reconnectTimer: ReturnType<typeof setTimeout> | undefined;
    const connect = () => { socket = new WebSocket(webSocketUrl()); socket.onopen = () => setIsLive(true); socket.onclose = () => { setIsLive(false); reconnectTimer = setTimeout(connect, 3_000); }; socket.onmessage = (event) => { try { if (JSON.parse(event.data).event_type === "telemetry_updated") void refresh(); } catch { /* malformed event is ignored */ } }; };
    connect();
    return () => { if (reconnectTimer) clearTimeout(reconnectTimer); socket?.close(); };
  }, []);
  const durationData = useMemo(() => Object.values(data.series.flatMap((series) => series.metrics.map((metric) => ({ time: new Date(metric.recorded_at).toLocaleTimeString(), [series.device_id]: metric.duration_ms }))).reduce<Record<string, Record<string, string | number>>>((points, point) => ({ ...points, [String(point.time)]: { ...(points[String(point.time)] ?? {}), ...point } }), {})), [data.series]);
  const latencyBuckets = useMemo(() => { const buckets = [{ name: "<100ms", value: 0 }, { name: "100–500ms", value: 0 }, { name: "500ms–1s", value: 0 }, { name: ">1s", value: 0 }]; data.series.flatMap((series) => series.metrics).forEach((metric) => { buckets[metric.latency_ms < 100 ? 0 : metric.latency_ms < 500 ? 1 : metric.latency_ms < 1000 ? 2 : 3].value += 1; }); return buckets; }, [data.series]);
  const taskData = useMemo(() => Object.entries(data.series.flatMap((series) => series.metrics).reduce<Record<string, number>>((counts, metric) => ({ ...counts, [metric.task_type]: (counts[metric.task_type] ?? 0) + 1 }), {})).map(([name, value]) => ({ name, value })), [data.series]);
  const powerData = useMemo(() => data.series.map((series) => series.metrics.reduce<Record<string, string | number>>((row, metric) => ({ ...row, [metric.task_type]: Number(row[metric.task_type] ?? 0) + metric.power_mwh }), { device: series.device_id })), [data.series]);
  const chartColors = ["#2563eb", "#7c3aed", "#db2777", "#059669"];
  return <section className="space-y-8"><div className="flex items-center justify-between"><div><h1 className="text-3xl font-bold">{t("telemetryDashboard")}</h1><p className="text-muted">{t("privacySafeAiPerformanceAcross")}</p></div><span className={`rounded-full px-3 py-1 text-sm font-medium ${isLive ? "bg-success-soft text-success" : "bg-surface-muted text-muted"}`}>{isLive ? t("live") : t("reconnecting")}</span></div><div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4"><Summary label={t("transcriptionTime")} value={formatDuration(data.summary.total_transcription_time_ms)} /><Summary label={t("averageLatency")} value={`${(data.summary.average_latency_ms / 1000).toFixed(2)} s`} /><Summary label={t("tasksProcessed")} value={String(data.summary.total_tasks)} /><Summary label={t("estimatedPower")} value={t("wh", { watthours: (data.summary.power_estimate_mwh / 1000).toFixed(2) })} /></div><div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">{data.devices.map((device) => <article className="rounded-xl border border-line bg-surface p-4 shadow-sm" key={device.device_id}><h2 className="font-semibold">{device.device_id}</h2><p className={statusClass(device.last_seen)}>{t("lastSeen", { time: new Date(device.last_seen).toLocaleString() })}</p><p>{t("vad", { support: device.supports_vad ? t("supported") : t("unavailable") })}</p><p className="text-sm text-muted">{device.supported_models.join(", ") || t("noModelsReported")}</p></article>)}</div><div className="grid gap-6 xl:grid-cols-2"><Chart title={t("transcriptionDurationOverTime")}><LineChart data={durationData}><XAxis dataKey="time" /><YAxis /><Tooltip /><Legend />{data.devices.map((device, index) => <Line dataKey={device.device_id} key={device.device_id} name={device.device_id} stroke={chartColors[index % chartColors.length]} />)}</LineChart></Chart><Chart title={t("latencyDistribution")}><BarChart data={latencyBuckets}><XAxis dataKey="name" /><YAxis allowDecimals={false} /><Tooltip /><Bar dataKey="value" fill="#7c3aed" /></BarChart></Chart><Chart title={t("taskTypeBreakdown")}><PieChart><Pie data={taskData} dataKey="value" nameKey="name" outerRadius={100}>{taskData.map((entry, index) => <Cell key={entry.name} fill={chartColors[index % chartColors.length]} />)}</Pie><Tooltip /><Legend /></PieChart></Chart><Chart title={t("powerConsumptionEstimate")}><BarChart data={powerData}><XAxis dataKey="device" /><YAxis /><Tooltip /><Legend />{["transcription", "cloud_transcription", "file_transfer", "diarization"].map((taskType, index) => <Bar dataKey={taskType} key={taskType} stackId="power" fill={chartColors[index]} />)}</BarChart></Chart></div></section>;
}

function Summary({ label, value }: { label: string; value: string }) { return <article className="rounded-xl bg-emphasis p-5 text-on-emphasis"><p className="text-sm text-on-emphasis-muted">{label}</p><p className="mt-2 text-3xl font-bold">{value}</p></article>; }
function Chart({ title, children }: { title: string; children: React.ReactElement }) { return <article className="h-80 rounded-xl border border-line bg-surface p-4 shadow-sm"><h2 className="mb-3 font-semibold">{title}</h2><ResponsiveContainer width="100%" height="90%">{children}</ResponsiveContainer></article>; }
function formatDuration(milliseconds: number) { const totalMinutes = Math.floor(milliseconds / 60_000); return `${String(Math.floor(totalMinutes / 60)).padStart(2, "0")}:${String(totalMinutes % 60).padStart(2, "0")}`; }
function statusClass(lastSeen: number) { const age = Date.now() - lastSeen; return `my-2 text-sm font-medium ${age <= 300_000 ? "text-success" : age <= 1_800_000 ? "text-warning" : "text-danger"}`; }
