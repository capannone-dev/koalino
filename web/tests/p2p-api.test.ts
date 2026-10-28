// Verifies the API client's device sync calls (Issue #43): reading the P2P status and each
// pairing step, with HTTP failures mapped to kinds the Devices page can word.

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import {
  acceptPairingRequest,
  cancelPairing,
  confirmPairingCode,
  getP2pStatus,
  makeDiscoverable,
  P2pRequestError,
  rejectPairingRequest,
  removeDevice,
  startPairing
} from "@/lib/p2p-api";

const fetchMock = vi.fn();
const peerId = "12D3KooWAbc123";

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

test("getP2pStatus reads the status without caching", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ enabled: false }), { status: 200 }));

  await expect(getP2pStatus()).resolves.toEqual({ enabled: false });
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/p2p/status", { cache: "no-store" });
});

test("startPairing posts the peer and returns the code", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ code: "639210" }), { status: 200 }));

  await expect(startPairing(peerId)).resolves.toBe("639210");
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/p2p/pairings", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ peer_id: peerId })
  });
});

test("confirming and accepting return the pairing update", async () => {
  const paired = { kind: "paired", peer_id: peerId, device_name: "Laptop" };
  fetchMock.mockImplementation(async () => new Response(JSON.stringify(paired), { status: 200 }));

  await expect(confirmPairingCode(peerId)).resolves.toEqual(paired);
  await expect(acceptPairingRequest(peerId)).resolves.toEqual(paired);
  expect(fetchMock.mock.calls.map(([url, init]) => [url, init.method])).toEqual([
    [`http://127.0.0.1:8080/api/p2p/pairings/${peerId}/confirm`, "POST"],
    [`http://127.0.0.1:8080/api/p2p/requests/${peerId}/accept`, "POST"]
  ]);
});

test("the steps without a body call their routes", async () => {
  fetchMock.mockImplementation(async () => new Response(null, { status: 204 }));

  await makeDiscoverable();
  await cancelPairing(peerId);
  await rejectPairingRequest(peerId);
  await removeDevice(peerId);

  expect(fetchMock.mock.calls.map(([url, init]) => [url, init.method])).toEqual([
    ["http://127.0.0.1:8080/api/p2p/pairable", "POST"],
    [`http://127.0.0.1:8080/api/p2p/pairings/${peerId}/cancel`, "POST"],
    [`http://127.0.0.1:8080/api/p2p/requests/${peerId}/reject`, "POST"],
    [`http://127.0.0.1:8080/api/p2p/devices/${peerId}`, "DELETE"]
  ]);
});

test("peer IDs are encoded into the path", async () => {
  fetchMock.mockResolvedValue(new Response(null, { status: 204 }));

  await removeDevice("a/b");

  expect(fetchMock.mock.calls[0][0]).toBe("http://127.0.0.1:8080/api/p2p/devices/a%2Fb");
});

test.each([
  [400, "invalid_peer"],
  [404, "not_found"],
  [409, "refused"],
  [503, "unavailable"],
  [500, "server"]
])("a %i answer is a %s failure", async (status, kind) => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ error: "detail" }), { status }));

  await expect(cancelPairing(peerId)).rejects.toMatchObject({ kind, status });
});

test("an unreachable daemon is a network failure", async () => {
  fetchMock.mockRejectedValue(new TypeError("fetch failed"));

  const failure = await getP2pStatus().catch((error: unknown) => error);

  expect(failure).toBeInstanceOf(P2pRequestError);
  expect(failure).toMatchObject({ kind: "network", status: null });
});
