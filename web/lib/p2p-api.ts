// Typed client for the daemon's device sync routes, `/api/p2p/*` (Issue #43): the P2P status the
// Devices page polls, and the pairing steps (make discoverable, pair, confirm or cancel, accept or
// reject a request, remove a paired device). The JSON shapes match `core/application/p2p_view.rs`.

import { apiBaseUrl } from "@/lib/api";

/**
 * What a device advertises in its manifest (REQUIREMENTS 3.4): CPU cores, GPU memory in
 * megabytes (0 when none is known), transcription model labels such as `whisper` or `openai`,
 * and whether it can split audio at speech pauses.
 */
export type DeviceCapabilities = {
  cpu_cores: number;
  gpu_vram_megabytes: number;
  transcription_models: string[];
  supports_vad: boolean;
};
/**
 * A paired device. `paired_at_ms` is Unix milliseconds; `capabilities` is `null` while the device
 * is not connected.
 */
export type TrustedDevice = {
  peer_id: string;
  device_name: string;
  paired_at_ms: number;
  /** Whether the device is connected right now (#153); absent from servers older than #153. */
  connected?: boolean;
  /** When it last synced, in Unix milliseconds; `null` (or absent) when it never has (#153). */
  last_synced_at_ms?: number | null;
  capabilities: DeviceCapabilities | null;
};
/** An unpaired device seen on the local network. */
/** `device_name` is present once the device told it; it tells only while discoverable (#121). */
export type NearbyDevice = { peer_id: string; device_name?: string | null };
/** A pairing request awaiting the owner's answer, with the code both devices show. */
export type IncomingPairingRequest = { peer_id: string; device_name: string; code: string };
/** A pairing this server started; `locally_confirmed` once the owner said the codes match. */
export type OutgoingPairing = { peer_id: string; code: string; locally_confirmed: boolean };

/** The result of a pairing step, tagged by `kind`. */
export type PairingUpdate =
  | { kind: "incoming_request"; peer_id: string; device_name: string; code: string }
  | { kind: "awaiting_remote"; peer_id: string }
  | { kind: "paired"; peer_id: string; device_name: string }
  | { kind: "rejected"; peer_id: string }
  | { kind: "failed"; peer_id: string };

/** `GET /api/p2p/status`: only `enabled` when device sync is off. */
export type P2pStatus =
  | { enabled: false }
  | {
      enabled: true;
      local_peer_id: string;
      device_name: string;
      /** Where recordings are transcribed: `local`, `auto`, or a paired device's peer ID. */
      transcription_route: string;
      listen_addresses: string[];
      capabilities: DeviceCapabilities;
      trusted_devices: TrustedDevice[];
      nearby_devices: NearbyDevice[];
      incoming_requests: IncomingPairingRequest[];
      outgoing_pairings: OutgoingPairing[];
      recent_updates: PairingUpdate[];
      /** Routes to other networks. */
      network: NetworkRoutes;
    };

/** This device's relay and static peers, from `NOTE_TAKER_P2P_RELAY` and `NOTE_TAKER_P2P_PEERS`. */
export type NetworkRoutes = {
  relay_address: string | null;
  relay_state: "not_configured" | "not_paired" | "connecting" | "connected" | "reserved";
  peers: string[];
  /** Whether this server is itself a relay for the owner's other devices. */
  offers_relay: boolean;
};
/** An invite to use on another device, and when it ends in Unix milliseconds. */
export type PairingInvite = { invite: string; expires_at_ms: number };
/** Why an invite could not be made or used. */
export type InviteFailureCode = "invite_malformed" | "invite_expired" | "invite_from_this_device" | "already_paired" | "no_addresses" | "unavailable" | "other";

/** Thrown by the invite calls; its code picks the message. */
export class InviteError extends Error {
  constructor(readonly code: InviteFailureCode) {
    super(`Invite request failed: ${code}`);
    this.name = "InviteError";
  }
}

/**
 * Why a device sync request failed: `invalid_peer` (400), `not_found` (404, not paired), `refused`
 * (409, for example a pairing no longer in progress), `unavailable` (503, sync off or stopped),
 * `server` (any other status), or `network` (the daemon was unreachable).
 */
export type P2pFailureKind = "invalid_peer" | "not_found" | "refused" | "unavailable" | "server" | "network";

/** Thrown by every call here; `status` is the HTTP status, or `null` for `network`. */
export class P2pRequestError extends Error {
  constructor(
    readonly kind: P2pFailureKind,
    readonly status: number | null
  ) {
    super(`Device sync request failed (${status ?? "network"}): ${kind}`);
    this.name = "P2pRequestError";
  }
}

/** Maps a non-2xx status to its failure kind. */
function failureKind(status: number): P2pFailureKind {
  if (status === 400) return "invalid_peer";
  if (status === 404) return "not_found";
  if (status === 409) return "refused";
  if (status === 503) return "unavailable";
  return "server";
}

/**
 * Sends one request, mapping an unreachable daemon and non-2xx answers to `P2pRequestError`.
 *
 * Inputs: the path under the API base URL and the fetch options. Outputs: the 2xx response.
 * Side effects: one HTTP request. The server's error text is dropped: it is not for display.
 */
async function p2pRequest(path: string, init: RequestInit): Promise<Response> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, init);
  } catch {
    throw new P2pRequestError("network", null);
  }
  if (!response.ok) throw new P2pRequestError(failureKind(response.status), response.status);
  return response;
}

/** Sends a body-less `POST`. */
const post = (path: string) => p2pRequest(path, { method: "POST" });
/** Encodes a peer ID as one path segment. */
const segment = (peerId: string) => encodeURIComponent(peerId);

/**
 * Reads the device sync state through `GET /api/p2p/status`.
 *
 * Outputs: the status. Throws `P2pRequestError`. Side effects: none.
 */
export async function getP2pStatus(): Promise<P2pStatus> {
  const response = await p2pRequest("/api/p2p/status", { cache: "no-store" });
  return (await response.json()) as P2pStatus;
}

/**
 * Lets other devices ask to pair for a few minutes (`POST /api/p2p/pairable`).
 *
 * Throws `P2pRequestError`. Side effects: opens the daemon's pairing window.
 */
export async function makeDiscoverable(): Promise<void> {
  await post("/api/p2p/pairable");
}

/**
 * Starts pairing with a nearby device (`POST /api/p2p/pairings`).
 *
 * Inputs: the nearby device's peer ID. Outputs: the six-digit code to compare on both devices.
 * Throws `P2pRequestError`. Side effects: the daemon dials the device and starts the exchange.
 */
export async function startPairing(peerId: string): Promise<string> {
  const response = await p2pRequest("/api/p2p/pairings", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ peer_id: peerId })
  });
  return ((await response.json()) as { code: string }).code;
}

/**
 * Tells the daemon the codes match for a pairing it started.
 *
 * Inputs: the peer ID. Outputs: the pairing update (`awaiting_remote` or `paired`, for example).
 * Throws `P2pRequestError`. Side effects: may add the device to the allowlist.
 */
export async function confirmPairingCode(peerId: string): Promise<PairingUpdate> {
  const response = await post(`/api/p2p/pairings/${segment(peerId)}/confirm`);
  return (await response.json()) as PairingUpdate;
}

/**
 * Cancels a pairing this server started. Throws `P2pRequestError`. Side effects: ends the exchange.
 */
export async function cancelPairing(peerId: string): Promise<void> {
  await post(`/api/p2p/pairings/${segment(peerId)}/cancel`);
}

/**
 * Accepts another device's pairing request.
 *
 * Inputs: the requesting peer ID. Outputs: the pairing update. Throws `P2pRequestError`.
 * Side effects: may add the device to the allowlist.
 */
export async function acceptPairingRequest(peerId: string): Promise<PairingUpdate> {
  const response = await post(`/api/p2p/requests/${segment(peerId)}/accept`);
  return (await response.json()) as PairingUpdate;
}

/**
 * Rejects another device's pairing request. Throws `P2pRequestError`. Side effects: ends the exchange.
 */
export async function rejectPairingRequest(peerId: string): Promise<void> {
  await post(`/api/p2p/requests/${segment(peerId)}/reject`);
}

/**
 * Removes a paired device (`DELETE /api/p2p/devices/{peer_id}`).
 *
 * Throws `P2pRequestError` (`not_found` when it was not paired). Side effects: the daemon drops it
 * from the allowlist and stops syncing with it.
 */
export async function removeDevice(peerId: string): Promise<void> {
  await p2pRequest(`/api/p2p/devices/${segment(peerId)}`, { method: "DELETE" });
}

/**
 * Chooses where recordings are transcribed (`PUT /api/p2p/transcription-route`).
 *
 * Inputs: `local`, `auto`, or a paired device's peer ID. Outputs: nothing.
 * Side effects: one HTTP request; throws `P2pRequestError` when it is refused.
 */
export async function setTranscriptionRoute(route: string): Promise<void> {
  await p2pRequest("/api/p2p/transcription-route", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ route })
  });
}

/**
 * Renames this device through `PUT /api/p2p/device-name` (#121).
 *
 * Inputs: the name as typed; the server trims and checks it. Outputs: the saved name.
 * Side effects: one HTTP request; the server saves the name and announces it at once. Throws
 * `P2pRequestError`, `invalid_peer` (400) for a blank, too long or control-character name.
 */
export async function setDeviceName(name: string): Promise<string> {
  const response = await p2pRequest("/api/p2p/device-name", {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name })
  });
  return ((await response.json()) as { device_name: string }).device_name;
}

/**
 * Sends an invite request, mapping refusals to `InviteError`.
 *
 * Inputs: the route and the JSON body. Outputs: the parsed 2xx answer. Side effects: one HTTP
 * request.
 */
async function inviteRequest<T>(path: string, body: object): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body)
    });
  } catch {
    throw new P2pRequestError("network", null);
  }
  if (response.ok) return (await response.json()) as T;
  const known: InviteFailureCode[] = ["invite_malformed", "invite_expired", "invite_from_this_device", "already_paired", "no_addresses", "unavailable"];
  const code = ((await response.json().catch(() => ({}))) as { error_code?: InviteFailureCode }).error_code;
  throw new InviteError(code && known.includes(code) ? code : "other");
}

/**
 * Makes an invite through `POST /api/p2p/invites`.
 *
 * Outputs: the invite. Throws `InviteError` or `P2pRequestError`.
 * Side effects: the daemon accepts pairing requests until it ends; pairing still needs the code.
 */
export const createInvite = () => inviteRequest<PairingInvite>("/api/p2p/invites", {});

/**
 * Passes another device's invite to `POST /api/p2p/invites/join`.
 *
 * Inputs: invite text as pasted, untrusted. Outputs: the device, now listed as nearby. Throws
 * `InviteError` or `P2pRequestError`. Side effects: no trust is granted.
 */
export const joinInvite = (invite: string) => inviteRequest<{ peer_id: string }>("/api/p2p/invites/join", { invite });
