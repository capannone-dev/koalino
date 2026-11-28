// Renders the web Devices page's body (Issue #43): this server's name, making it discoverable,
// pairing with nearby devices by comparing a six-digit code, answering pairing requests, and
// removing paired devices. It polls `GET /api/p2p/status` every two seconds, as the Flutter
// Devices screen does, and words every failure itself: the server's error text is never shown.
// Nearby devices show the name they told, and this server can be renamed (Issue #121).

"use client";

import { useCallback, useEffect, useState } from "react";
import {
  acceptPairingRequest,
  cancelPairing,
  confirmPairingCode,
  createInvite,
  getP2pStatus,
  InviteError,
  joinInvite,
  makeDiscoverable,
  P2pRequestError,
  rejectPairingRequest,
  removeDevice,
  setDeviceName,
  startPairing,
  type DeviceCapabilities,
  type InviteFailureCode,
  type PairingInvite,
  type P2pFailureKind,
  type P2pStatus,
  type PairingUpdate,
  type TrustedDevice
} from "@/lib/p2p-api";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";
import { fieldStyle } from "@/components/ui";
import { EmptyState } from "@/components/ui";

/** How often the status is read again, in milliseconds. */
const statusPollIntervalMs = 2_000;
/** How long the daemon accepts pairing requests after Make discoverable (`PAIRING_WINDOW`). */
const pairingWindowMs = 5 * 60 * 1_000;
/** Shown whenever the daemon cannot be reached. */
const unreachableMessage = (t: Translate) => t("cantReachTheNoteTaking");

/** Plain messages per failure kind. */
const failureMessages = (t: Translate) : Record<P2pFailureKind, string> => ({
  refused: t("theOtherDeviceIsNo"),
  unavailable: t("deviceSyncIsNotRunning"),
  network: unreachableMessage(t),
  invalid_peer: t("thatDidNotWorkTry"),
  not_found: t("thatDidNotWorkTry"),
  server: t("thatDidNotWorkTry")
});

/** Plain messages per invite refusal. */
const inviteMessages = (t: Translate) : Record<InviteFailureCode, string> => ({
  invite_malformed: t("thatIsNotAValid"),
  invite_expired: t("thisInviteHasExpiredMake"),
  invite_from_this_device: t("thisInviteWasMadeOn"),
  already_paired: t("thatDeviceIsAlreadyPaired"),
  no_addresses: t("thisServerIsNotOn"),
  unavailable: t("deviceSyncIsNotRunning"),
  other: t("thatDidNotWorkTry")
});

/** Explains an empty Nearby list: it only covers this network, and what to do otherwise. */
export function describeEmptyNearby(t: Translate, hasRelay: boolean): string {
  const nextStep = hasRelay ? t("useAnInvite") : t("useAnInviteOrGive");
  return t("noOtherDevicesFoundOn", { nextStep });
}

/** Explains what a paired device that is not connected needs to reconnect. */
export function describeDisconnectedDevice(t: Translate, hasRelay: boolean): string {
  return hasRelay
    ? t("notConnectedItReconnectsWhen")
    : t("notConnectedPutBothDevices");
}

/** Words any thrown value as a failure message. */
function failureMessage(t: Translate, error: unknown): string {
  if (error instanceof InviteError) return inviteMessages(t)[error.code];
  return failureMessages(t)[error instanceof P2pRequestError ? error.kind : "server"];
}

/** Words the result of confirming or accepting a pairing; `null` while it is still under way. */
function pairingResultMessage(t: Translate, update: PairingUpdate): string | null {
  switch (update.kind) {
    case "paired":
      return t("pairedWith", { deviceName: update.device_name });
    case "rejected":
      return t("theOtherDeviceRejectedThe");
    case "failed":
      return t("pairingFailedTryAgain");
    default:
      return null;
  }
}

/** Shortens a peer ID to `12D3Koo…1234` form; devices are told apart by their tail. */
export function shortPeerId(peerId: string): string {
  return peerId.length <= 16 ? peerId : `${peerId.slice(0, 7)}…${peerId.slice(-6)}`;
}

/** Splits a six-digit code into two groups of three, such as `639 210`. */
export function formatPairingCode(code: string): string {
  return code.length === 6 ? `${code.slice(0, 3)} ${code.slice(3)}` : code;
}

/** Formats a pairing time (Unix milliseconds) in local time as `28 Oct 2026`. */
export function formatPairedDate(pairedAtMs: number): string {
  return new Date(pairedAtMs).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });
}

/** Display names of known transcription model labels; others are shown as given. */
const modelNames = (t: Translate) : Record<string, string> => ({ whisper: t("whisperOnDevice"), openai: t("openaiCloud") });

/** Summarizes a device's manifest as `8 CPU cores · 4 GB GPU · Whisper (on device)`. */
export function describeCapabilities(t: Translate, capabilities: DeviceCapabilities): string {
  const parts = [`${capabilities.cpu_cores} CPU ${capabilities.cpu_cores === 1 ? "core" : "cores"}`];
  const gpuMegabytes = capabilities.gpu_vram_megabytes;
  if (gpuMegabytes > 0) {
    parts.push(gpuMegabytes >= 1_024 ? t("gbGpu", { gigabytes: Math.round(gpuMegabytes / 1_024) }) : t("mbGpu", { gpuMegabytes }));
  }
  parts.push(
    capabilities.transcription_models.length === 0
      ? t("noTranscriptionModels")
      : capabilities.transcription_models.map((model) => modelNames(t)[model] ?? model).join(", ")
  );
  return parts.join(" · ");
}

/**
 * Says when a paired device last synced, as `Last synced 5 minutes ago` (#153).
 *
 * Inputs: the last sync time in Unix milliseconds (`null`/absent for never) and the current time.
 * Outputs: never; just now under a minute or for a time ahead of `nowMs`; minutes under an hour;
 * hours under a day; then the local date and time. Side effects: none.
 */
export function describeLastSynced(t: Translate, lastSyncedAtMs: number | null | undefined, nowMs: number): string {
  if (lastSyncedAtMs === null || lastSyncedAtMs === undefined) return t("neverSynced");
  const sinceSyncMs = nowMs - lastSyncedAtMs;
  const minuteMs = 60_000;
  if (sinceSyncMs < minuteMs) return t("syncedJustNow");
  if (sinceSyncMs < 60 * minuteMs) return t("lastSyncedMinutesAgo", { count: Math.floor(sinceSyncMs / minuteMs) });
  if (sinceSyncMs < 24 * 60 * minuteMs) return t("lastSyncedHoursAgo", { count: Math.floor(sinceSyncMs / (60 * minuteMs)) });
  const date = new Date(lastSyncedAtMs).toLocaleString("en-GB", { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
  return t("lastSyncedOn", { date });
}

const primaryButton = "min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled";
const secondaryButton = "min-h-11 rounded-lg border border-line-strong px-4 font-medium hover:bg-canvas disabled:text-faint";

export type DevicesPanelProps = {
  /** The status read by the server page, or `null` when the daemon could not be reached. */
  initialStatus: P2pStatus | null;
};

/**
 * Shows device sync and its pairing steps.
 *
 * Inputs: see `DevicesPanelProps`. Outputs: the Devices page body with a status line announced to
 * screen readers. Side effects: polls `GET /api/p2p/status` every two seconds while mounted, and
 * calls the pairing routes when the owner acts; each action is followed by a status read.
 */
export function DevicesPanel({ initialStatus }: DevicesPanelProps) {
  const t = useT();
  const [status, setStatus] = useState(initialStatus);
  const [isReachable, setIsReachable] = useState(initialStatus !== null);
  const [message, setMessage] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [discoverableUntilMs, setDiscoverableUntilMs] = useState<number | null>(null);
  const [deviceToRemove, setDeviceToRemove] = useState<TrustedDevice | null>(null);
  const [invite, setInvite] = useState<PairingInvite | null>(null);
  const [inviteText, setInviteText] = useState("");
  /** The name being typed, or `null` while the rename form is closed. */
  const [draftName, setDraftName] = useState<string | null>(null);
  const [renameRefusal, setRenameRefusal] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await getP2pStatus());
      setIsReachable(true);
    } catch {
      setIsReachable(false);
    }
  }, []);

  useEffect(() => {
    const timer = setInterval(() => void refreshStatus(), statusPollIntervalMs);
    return () => clearInterval(timer);
  }, [refreshStatus]);

  /** Runs one pairing step, words its outcome, and reads the status again. */
  async function runStep(step: () => Promise<string | null>) {
    setIsBusy(true);
    setMessage(null);
    try {
      setMessage(await step());
    } catch (error) {
      setMessage(failureMessage(t, error));
    } finally {
      setIsBusy(false);
    }
    await refreshStatus();
  }

  /** Saves the typed name; a refusal stays beside the field, which stays open. */
  async function saveName(name: string) {
    setIsBusy(true);
    setRenameRefusal(null);
    try {
      await setDeviceName(name);
      setDraftName(null);
      setMessage(null);
    } catch (error) {
      setRenameRefusal(
        error instanceof P2pRequestError && error.kind === "invalid_peer" ? t("use1To64Characters") : failureMessage(t, error)
      );
      setIsBusy(false);
      return;
    }
    setIsBusy(false);
    await refreshStatus();
  }

  const statusLine = (
    <p role="status" aria-live="polite">
      {isReachable ? message : unreachableMessage(t)}
    </p>
  );

  if (status === null) return statusLine;
  if (!status.enabled) {
    return (
      <div className="card max-w-2xl space-y-2">
        <h2 className="text-lg font-semibold">{t("deviceSyncIsOff")}</h2>
        <p className="text-muted">{t("meetingsStayOnThisServer")}</p>
        {statusLine}
      </div>
    );
  }

  const hasRelay = status.network.relay_address !== null || status.network.offers_relay;
  const isDiscoverable = discoverableUntilMs !== null && Date.now() < discoverableUntilMs;

  return (
    <div className="max-w-2xl space-y-6">
      {statusLine}
      <div className="card space-y-3">
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="font-semibold">{status.device_name}</p>
            <p className="text-sm text-muted">{t("thisDevice")}</p>
            <p className="text-sm text-muted">{describeCapabilities(t, status.capabilities)}</p>
          </div>
          {draftName === null && (
            <button
              type="button"
              className={secondaryButton}
              aria-label={t("renameThisDevice")}
              disabled={isBusy}
              onClick={() => {
                setRenameRefusal(null);
                setDraftName(status.device_name);
              }}
            >{t("rename")}</button>
          )}
        </div>
        {draftName !== null && (
          <form
            className="space-y-2"
            onSubmit={(event) => {
              event.preventDefault();
              void saveName(draftName);
            }}
          >
            <label className="block">{t("deviceName")}<input
                className={fieldStyle}
                type="text"
                autoComplete="off"
                maxLength={256}
                value={draftName}
                aria-invalid={renameRefusal !== null}
                aria-describedby="device-name-help"
                onChange={(event) => {
                  setDraftName(event.target.value);
                  setRenameRefusal(null);
                }}
              />
            </label>
            <p id="device-name-help" className={renameRefusal ? "text-sm text-danger" : "text-sm text-muted"}>
              {renameRefusal ?? t("nearbyDevicesSeeThisName")}
            </p>
            <div className="flex gap-2">
              <button type="submit" className={primaryButton} disabled={isBusy}>{t("save")}</button>
              <button type="button" className={secondaryButton} disabled={isBusy} onClick={() => setDraftName(null)}>{t("cancel")}</button>
            </div>
          </form>
        )}
        {isDiscoverable ? (
          <p>{t("discoverableYourOtherDevicesCan")}</p>
        ) : (
          <button
            type="button"
            className={secondaryButton}
            disabled={isBusy}
            onClick={() =>
              void runStep(async () => {
                await makeDiscoverable();
                setDiscoverableUntilMs(Date.now() + pairingWindowMs);
                return null;
              })
            }
          >{t("makeDiscoverable")}</button>
        )}
      </div>

      {status.incoming_requests.length > 0 && (
        <DeviceSection title={t("pairingRequests")}>
          {status.incoming_requests.map((request) => (
            <CodeCard key={request.peer_id} title={request.device_name} code={request.code} instructions={t("acceptOnlyIfTheOther")}>
              <button type="button" className={secondaryButton} disabled={isBusy} onClick={() => void runStep(async () => (await rejectPairingRequest(request.peer_id), null))}>{t("reject")}</button>
              <button type="button" className={primaryButton} disabled={isBusy} onClick={() => void runStep(async () => pairingResultMessage(t, await acceptPairingRequest(request.peer_id)))}>{t("accept")}</button>
            </CodeCard>
          ))}
        </DeviceSection>
      )}

      {status.outgoing_pairings.length > 0 && (
        <DeviceSection title={t("pairingInProgress")}>
          {status.outgoing_pairings.map((pairing) => (
            <CodeCard
              key={pairing.peer_id}
              title={shortPeerId(pairing.peer_id)}
              code={pairing.code}
              instructions={pairing.locally_confirmed ? t("waitingForTheOtherDevice") : t("checkThatTheOtherDevice")}
            >
              <button type="button" className={secondaryButton} disabled={isBusy} onClick={() => void runStep(async () => (await cancelPairing(pairing.peer_id), null))}>{t("cancelPairing")}</button>
              {!pairing.locally_confirmed && (
                <button type="button" className={primaryButton} disabled={isBusy} onClick={() => void runStep(async () => pairingResultMessage(t, await confirmPairingCode(pairing.peer_id)))}>{t("codesMatch")}</button>
              )}
            </CodeCard>
          ))}
        </DeviceSection>
      )}

      <DeviceSection
        title={t("pairedDevices")}
        note={
          status.trusted_devices.length === 0
            ? undefined
            : t("devicesOnlineCount", { online: status.trusted_devices.filter((device) => device.connected).length, total: status.trusted_devices.length })
        }
        action={
          <button type="button" className={secondaryButton} onClick={() => void refreshStatus()}>
            {t("refresh")}
          </button>
        }
      >
        {status.trusted_devices.length === 0 && <EmptyState icon="devices" title={t("noPairedDevicesYet")} />}
        {status.trusted_devices.map((device) => (
          <div key={device.peer_id} className="card space-y-2">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold">{device.device_name}</span>
                  <ConnectionBadge online={device.connected === true} />
                </p>
                <p className="text-sm">{describeLastSynced(t, device.last_synced_at_ms, Date.now())}</p>
                <p className="text-sm text-muted">{t("paired", { pairedDate: formatPairedDate(device.paired_at_ms) })}</p>
                {/* A device can be connected a moment before its manifest arrives. */}
                {(device.capabilities || !device.connected) && (
                  <p className="text-sm text-muted">{device.capabilities ? describeCapabilities(t, device.capabilities) : describeDisconnectedDevice(t, hasRelay)}</p>
                )}
              </div>
              {deviceToRemove?.peer_id !== device.peer_id && (
                <button type="button" className={secondaryButton} aria-label={t("remove2", { deviceName: device.device_name })} disabled={isBusy} onClick={() => setDeviceToRemove(device)}>{t("remove")}</button>
              )}
            </div>
            {deviceToRemove?.peer_id === device.peer_id && (
              <div className="space-y-2">
                <p>{t("stopsSyncingWithThisServer", { deviceName: device.device_name })}</p>
                <div className="flex justify-end gap-2">
                  <button type="button" className={secondaryButton} onClick={() => setDeviceToRemove(null)}>{t("cancel")}</button>
                  <button
                    type="button"
                    className={primaryButton}
                    disabled={isBusy}
                    onClick={() =>
                      void runStep(async () => {
                        setDeviceToRemove(null);
                        await removeDevice(device.peer_id);
                        return null;
                      })
                    }
                  >{t("remove")}</button>
                </div>
              </div>
            )}
          </div>
        ))}
      </DeviceSection>

      <DeviceSection title={t("nearbyDevices")}>
        {status.nearby_devices.length === 0 && (
          <p className="text-muted">{describeEmptyNearby(t, hasRelay)}</p>
        )}
        {status.nearby_devices.map((device) => (
          <div key={device.peer_id} className="card flex items-center justify-between gap-4">
            {/* Without a name, the short peer ID tells unnamed devices apart; a named device
                shows it only when another nearby device has the same name (#211). */}
            <div>
              <p className="font-semibold">{device.device_name || t("unnamedDevice")}</p>
              {!device.device_name || status.nearby_devices.some((other) => other.peer_id !== device.peer_id && other.device_name === device.device_name) ? (
                <p className="font-mono text-sm text-muted">{shortPeerId(device.peer_id)}</p>
              ) : null}
            </div>
            <button type="button" className={primaryButton} disabled={isBusy} onClick={() => void runStep(async () => (await startPairing(device.peer_id), null))}>{t("pair")}</button>
          </div>
        ))}
      </DeviceSection>

      <DeviceSection title={t("anotherNetwork")}>
        <p className="text-muted">{t("showAnInviteHereAnd")}</p>
        <button
          type="button"
          className={secondaryButton}
          disabled={isBusy}
          onClick={() =>
            void runStep(async () => {
              const created = await createInvite();
              setInvite(created);
              setDiscoverableUntilMs(created.expires_at_ms);
              return null;
            })
          }
        >{t("showInvite")}</button>
        {invite && (
          <div className="card space-y-2">
            <p className="break-all font-mono text-sm">{invite.invite}</p>
            <p className="text-sm text-muted">{t("worksUntil", { time: new Date(invite.expires_at_ms).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" }) })}</p>
            <button type="button" className={secondaryButton} onClick={() => void navigator.clipboard?.writeText(invite.invite)}>{t("copyLink")}</button>
          </div>
        )}
        <form
          className="space-y-2"
          onSubmit={(event) => {
            event.preventDefault();
            void runStep(async () => {
              await joinInvite(inviteText);
              setInviteText("");
              return t("deviceFoundChoosePairUnder");
            });
          }}
        >
          <label className="block">{t("inviteFromAnotherDevice")}<input className={fieldStyle} type="text" autoComplete="off" value={inviteText} onChange={(event) => setInviteText(event.target.value)} />
          </label>
          <button type="submit" className={secondaryButton} disabled={isBusy || inviteText.trim() === ""}>{t("findDevice")}</button>
        </form>
      </DeviceSection>
    </div>
  );
}

/** A titled section, exposed as a region named by its heading; `note` shows muted beside it, then `action`. */
function DeviceSection({ title, note, action, children }: { title: string; note?: string; action?: React.ReactNode; children: React.ReactNode }) {
  const headingId = `devices-${title.toLowerCase().replaceAll(" ", "-")}`;
  return (
    <section aria-labelledby={headingId} className="space-y-3">
      <div className="flex items-center gap-4">
        <h2 id={headingId} className="grow text-lg font-semibold">
          {title}
        </h2>
        {note && <p className="text-sm text-muted">{note}</p>}
        {action}
      </div>
      {children}
    </section>
  );
}

/** A pill reading `Online` (green dot) or `Offline` (grey dot) beside a paired device's name (#153). */
function ConnectionBadge({ online }: { online: boolean }) {
  const t = useT();
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full border px-2 text-sm ${online ? "border-current text-success" : "border-line-strong text-muted"}`}>
      <span aria-hidden="true" className="inline-block size-2 rounded-full bg-current" />
      {online ? t("deviceOnline") : t("deviceOffline")}
    </span>
  );
}

/** A card with a device name, a pairing code in large type, instructions, and actions. */
function CodeCard({ title, code, instructions, children }: { title: string; code: string; instructions: string; children: React.ReactNode }) {
  const t = useT();
  return (
    <div className="card space-y-2">
      <p className="font-semibold">{title}</p>
      <p className="text-3xl font-semibold tabular-nums tracking-widest" aria-label={t("pairingCode", { code: code.split("").join(" ") })}>
        {formatPairingCode(code)}
      </p>
      <p>{instructions}</p>
      <div className="flex justify-end gap-2">{children}</div>
    </div>
  );
}
