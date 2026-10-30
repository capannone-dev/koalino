// Renders the web Devices page's body (Issue #43): this server's name, making it discoverable,
// pairing with nearby devices by comparing a six-digit code, answering pairing requests, and
// removing paired devices. It polls `GET /api/p2p/status` every two seconds, as the Flutter
// Devices screen does, and words every failure itself: the server's error text is never shown.

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
  startPairing,
  type DeviceCapabilities,
  type InviteFailureCode,
  type PairingInvite,
  type P2pFailureKind,
  type P2pStatus,
  type PairingUpdate,
  type TrustedDevice
} from "@/lib/p2p-api";

/** How often the status is read again, in milliseconds. */
const statusPollIntervalMs = 2_000;
/** How long the daemon accepts pairing requests after Make discoverable (`PAIRING_WINDOW`). */
const pairingWindowMs = 5 * 60 * 1_000;
/** Shown whenever the daemon cannot be reached. */
const unreachableMessage = "Can't reach the note-taking server. Check that it is running.";

/** Plain messages per failure kind. */
const failureMessages: Record<P2pFailureKind, string> = {
  refused: "The other device is no longer pairing. Try again.",
  unavailable: "Device sync is not running.",
  network: unreachableMessage,
  invalid_peer: "That did not work. Try again.",
  not_found: "That did not work. Try again.",
  server: "That did not work. Try again."
};

/** Plain messages per invite refusal. */
const inviteMessages: Record<InviteFailureCode, string> = {
  invite_malformed: "That is not a valid invite. Copy it again.",
  invite_expired: "This invite has expired. Make a new one on the other device.",
  invite_from_this_device: "This invite was made on this device. Use it on the other one.",
  already_paired: "That device is already paired.",
  no_addresses: "This server is not on a network yet. Connect it and try again.",
  unavailable: "Device sync is not running.",
  other: "That did not work. Try again."
};

/** Explains an empty Nearby list: it only covers this network, and what to do otherwise. */
export function describeEmptyNearby(hasRelay: boolean): string {
  const nextStep = hasRelay ? "use an invite." : "use an invite, or give this server a relay with NOTE_TAKER_P2P_RELAY.";
  return `No other devices found on this network. Nearby only finds devices on the same Wi-Fi. For a device on another network, ${nextStep}`;
}

/** Explains what a paired device that is not connected needs to reconnect. */
export function describeDisconnectedDevice(hasRelay: boolean): string {
  return hasRelay
    ? "Not connected. It reconnects when it is online and can reach your relay."
    : "Not connected. Put both devices on the same Wi-Fi, or give this server a relay with NOTE_TAKER_P2P_RELAY.";
}

/** Words any thrown value as a failure message. */
function failureMessage(error: unknown): string {
  if (error instanceof InviteError) return inviteMessages[error.code];
  return failureMessages[error instanceof P2pRequestError ? error.kind : "server"];
}

/** Words the result of confirming or accepting a pairing; `null` while it is still under way. */
function pairingResultMessage(update: PairingUpdate): string | null {
  switch (update.kind) {
    case "paired":
      return `Paired with ${update.device_name}.`;
    case "rejected":
      return "The other device rejected the pairing.";
    case "failed":
      return "Pairing failed. Try again.";
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
const modelNames: Record<string, string> = { whisper: "Whisper (on device)", openai: "OpenAI (cloud)" };

/** Summarizes a device's manifest as `8 CPU cores · 4 GB GPU · Whisper (on device)`. */
export function describeCapabilities(capabilities: DeviceCapabilities): string {
  const parts = [`${capabilities.cpu_cores} CPU ${capabilities.cpu_cores === 1 ? "core" : "cores"}`];
  const gpuMegabytes = capabilities.gpu_vram_megabytes;
  if (gpuMegabytes > 0) {
    parts.push(gpuMegabytes >= 1_024 ? `${Math.round(gpuMegabytes / 1_024)} GB GPU` : `${gpuMegabytes} MB GPU`);
  }
  parts.push(
    capabilities.transcription_models.length === 0
      ? "No transcription models"
      : capabilities.transcription_models.map((model) => modelNames[model] ?? model).join(", ")
  );
  return parts.join(" · ");
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
  const [status, setStatus] = useState(initialStatus);
  const [isReachable, setIsReachable] = useState(initialStatus !== null);
  const [message, setMessage] = useState<string | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [discoverableUntilMs, setDiscoverableUntilMs] = useState<number | null>(null);
  const [deviceToRemove, setDeviceToRemove] = useState<TrustedDevice | null>(null);
  const [invite, setInvite] = useState<PairingInvite | null>(null);
  const [inviteText, setInviteText] = useState("");

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
      setMessage(failureMessage(error));
    } finally {
      setIsBusy(false);
    }
    await refreshStatus();
  }

  const statusLine = (
    <p role="status" aria-live="polite">
      {isReachable ? message : unreachableMessage}
    </p>
  );

  if (status === null) return statusLine;
  if (!status.enabled) {
    return (
      <div className="card max-w-2xl space-y-2">
        <h2 className="text-lg font-semibold">Device sync is off</h2>
        <p className="text-muted">Meetings stay on this server until sync is turned on.</p>
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
        <div>
          <p className="font-semibold">{status.device_name}</p>
          <p className="text-sm text-muted">This device</p>
          <p className="text-sm text-muted">{describeCapabilities(status.capabilities)}</p>
        </div>
        {isDiscoverable ? (
          <p>Discoverable: your other devices can ask to pair for a few minutes.</p>
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
          >
            Make discoverable
          </button>
        )}
      </div>

      {status.incoming_requests.length > 0 && (
        <DeviceSection title="Pairing requests">
          {status.incoming_requests.map((request) => (
            <CodeCard key={request.peer_id} title={request.device_name} code={request.code} instructions="Accept only if the other device shows the same code.">
              <button type="button" className={secondaryButton} disabled={isBusy} onClick={() => void runStep(async () => (await rejectPairingRequest(request.peer_id), null))}>
                Reject
              </button>
              <button type="button" className={primaryButton} disabled={isBusy} onClick={() => void runStep(async () => pairingResultMessage(await acceptPairingRequest(request.peer_id)))}>
                Accept
              </button>
            </CodeCard>
          ))}
        </DeviceSection>
      )}

      {status.outgoing_pairings.length > 0 && (
        <DeviceSection title="Pairing in progress">
          {status.outgoing_pairings.map((pairing) => (
            <CodeCard
              key={pairing.peer_id}
              title={shortPeerId(pairing.peer_id)}
              code={pairing.code}
              instructions={pairing.locally_confirmed ? "Waiting for the other device…" : "Check that the other device shows the same code."}
            >
              <button type="button" className={secondaryButton} disabled={isBusy} onClick={() => void runStep(async () => (await cancelPairing(pairing.peer_id), null))}>
                Cancel pairing
              </button>
              {!pairing.locally_confirmed && (
                <button type="button" className={primaryButton} disabled={isBusy} onClick={() => void runStep(async () => pairingResultMessage(await confirmPairingCode(pairing.peer_id)))}>
                  Codes match
                </button>
              )}
            </CodeCard>
          ))}
        </DeviceSection>
      )}

      <DeviceSection title="Paired devices">
        {status.trusted_devices.length === 0 && <p className="text-muted">No paired devices yet.</p>}
        {status.trusted_devices.map((device) => (
          <div key={device.peer_id} className="card space-y-2">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="font-semibold">{device.device_name}</p>
                <p className="text-sm text-muted">{`Paired ${formatPairedDate(device.paired_at_ms)}`}</p>
                <p className="text-sm text-muted">{device.capabilities ? describeCapabilities(device.capabilities) : describeDisconnectedDevice(hasRelay)}</p>
              </div>
              {deviceToRemove?.peer_id !== device.peer_id && (
                <button type="button" className={secondaryButton} aria-label={`Remove ${device.device_name}`} disabled={isBusy} onClick={() => setDeviceToRemove(device)}>
                  Remove
                </button>
              )}
            </div>
            {deviceToRemove?.peer_id === device.peer_id && (
              <div className="space-y-2">
                <p>{`${device.device_name} stops syncing with this server. Pair again to resume.`}</p>
                <div className="flex justify-end gap-2">
                  <button type="button" className={secondaryButton} onClick={() => setDeviceToRemove(null)}>
                    Cancel
                  </button>
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
                  >
                    Remove
                  </button>
                </div>
              </div>
            )}
          </div>
        ))}
      </DeviceSection>

      <DeviceSection title="Nearby devices">
        {status.nearby_devices.length === 0 && (
          <p className="text-muted">{describeEmptyNearby(hasRelay)}</p>
        )}
        {status.nearby_devices.map((device) => (
          <div key={device.peer_id} className="card flex items-center justify-between gap-4">
            <p className="font-mono">{shortPeerId(device.peer_id)}</p>
            <button type="button" className={primaryButton} disabled={isBusy} onClick={() => void runStep(async () => (await startPairing(device.peer_id), null))}>
              Pair
            </button>
          </div>
        ))}
      </DeviceSection>

      <DeviceSection title="Another network">
        <p className="text-muted">Show an invite here and use it on the other device. You still compare a code before they pair.</p>
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
        >
          Show invite
        </button>
        {invite && (
          <div className="card space-y-2">
            <p className="break-all font-mono text-sm">{invite.invite}</p>
            <p className="text-sm text-muted">{`Works until ${new Date(invite.expires_at_ms).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" })}.`}</p>
            <button type="button" className={secondaryButton} onClick={() => void navigator.clipboard?.writeText(invite.invite)}>
              Copy link
            </button>
          </div>
        )}
        <form
          className="space-y-2"
          onSubmit={(event) => {
            event.preventDefault();
            void runStep(async () => {
              await joinInvite(inviteText);
              setInviteText("");
              return "Device found. Choose Pair under Nearby devices.";
            });
          }}
        >
          <label className="block">
            Invite from another device
            <input className="mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30" type="text" autoComplete="off" value={inviteText} onChange={(event) => setInviteText(event.target.value)} />
          </label>
          <button type="submit" className={secondaryButton} disabled={isBusy || inviteText.trim() === ""}>
            Find device
          </button>
        </form>
      </DeviceSection>
    </div>
  );
}

/** A titled section, exposed as a region named by its heading. */
function DeviceSection({ title, children }: { title: string; children: React.ReactNode }) {
  const headingId = `devices-${title.toLowerCase().replaceAll(" ", "-")}`;
  return (
    <section aria-labelledby={headingId} className="space-y-3">
      <h2 id={headingId} className="text-lg font-semibold">
        {title}
      </h2>
      {children}
    </section>
  );
}

/** A card with a device name, a pairing code in large type, instructions, and actions. */
function CodeCard({ title, code, instructions, children }: { title: string; code: string; instructions: string; children: React.ReactNode }) {
  return (
    <div className="card space-y-2">
      <p className="font-semibold">{title}</p>
      <p className="text-3xl font-semibold tabular-nums tracking-widest" aria-label={`Pairing code ${code.split("").join(" ")}`}>
        {formatPairingCode(code)}
      </p>
      <p>{instructions}</p>
      <div className="flex justify-end gap-2">{children}</div>
    </div>
  );
}
