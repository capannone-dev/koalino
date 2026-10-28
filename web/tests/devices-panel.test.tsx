// Verifies the web Devices page (Issue #43): what it shows for each P2P state, each pairing step,
// removing a paired device, and the messages for failures.

import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { DevicesPanel } from "@/components/devices-panel";
import * as p2pApi from "@/lib/p2p-api";
import type { P2pStatus } from "@/lib/p2p-api";

vi.mock("@/lib/p2p-api", async () => ({
  ...(await vi.importActual<typeof p2pApi>("@/lib/p2p-api")),
  getP2pStatus: vi.fn(),
  makeDiscoverable: vi.fn(),
  startPairing: vi.fn(),
  confirmPairingCode: vi.fn(),
  cancelPairing: vi.fn(),
  acceptPairingRequest: vi.fn(),
  rejectPairingRequest: vi.fn(),
  removeDevice: vi.fn()
}));

const nearbyPeerId = "12D3KooWNearbyPeer000000000000000000000000000abcdef";
const pairedPeerId = "12D3KooWPairedPeer000000000000000000000000000123456";

/** A running status with one device of each kind, overridable per test. */
function runningStatus(overrides: Partial<Extract<P2pStatus, { enabled: true }>> = {}): P2pStatus {
  return {
    enabled: true,
    local_peer_id: "12D3KooWLocal",
    device_name: "Office server",
    listen_addresses: [],
    trusted_devices: [],
    nearby_devices: [],
    incoming_requests: [],
    outgoing_pairings: [],
    recent_updates: [],
    ...overrides
  };
}

beforeEach(() => {
  for (const mock of Object.values(p2pApi)) {
    if (vi.isMockFunction(mock)) mock.mockReset();
  }
  vi.mocked(p2pApi.getP2pStatus).mockResolvedValue(runningStatus());
});

afterEach(() => {
  vi.useRealTimers();
});

test("explains when device sync is off", () => {
  render(<DevicesPanel initialStatus={{ enabled: false }} />);

  expect(screen.getByRole("heading", { name: "Device sync is off" })).toBeInTheDocument();
  expect(screen.getByText("Meetings stay on this server until sync is turned on.")).toBeInTheDocument();
});

test("explains when the server cannot be reached", () => {
  render(<DevicesPanel initialStatus={null} />);

  expect(screen.getByText("Can't reach the note-taking server. Check that it is running.")).toBeInTheDocument();
});

test("shows this device and the empty sections", () => {
  render(<DevicesPanel initialStatus={runningStatus()} />);

  expect(screen.getByText("Office server")).toBeInTheDocument();
  expect(screen.getByText("No paired devices yet.")).toBeInTheDocument();
  expect(
    screen.getByText("No other devices found. Open Note Taker on another device on this network and make it discoverable.")
  ).toBeInTheDocument();
});

test("Make discoverable opens the pairing window", async () => {
  vi.mocked(p2pApi.makeDiscoverable).mockResolvedValue();
  render(<DevicesPanel initialStatus={runningStatus()} />);

  fireEvent.click(screen.getByRole("button", { name: "Make discoverable" }));

  expect(await screen.findByText("Discoverable: your other devices can ask to pair for a few minutes.")).toBeInTheDocument();
  expect(p2pApi.makeDiscoverable).toHaveBeenCalled();
});

test("pairing a nearby device shows the code to compare", async () => {
  vi.mocked(p2pApi.startPairing).mockResolvedValue("639210");
  vi.mocked(p2pApi.getP2pStatus).mockResolvedValue(
    runningStatus({ outgoing_pairings: [{ peer_id: nearbyPeerId, code: "639210", locally_confirmed: false }] })
  );
  render(<DevicesPanel initialStatus={runningStatus({ nearby_devices: [{ peer_id: nearbyPeerId }] })} />);

  expect(screen.getByText("12D3Koo…abcdef")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Pair" }));

  const card = await screen.findByRole("region", { name: "Pairing in progress" });
  expect(within(card).getByLabelText("Pairing code 6 3 9 2 1 0")).toHaveTextContent("639 210");
  expect(within(card).getByText("Check that the other device shows the same code.")).toBeInTheDocument();
  expect(p2pApi.startPairing).toHaveBeenCalledWith(nearbyPeerId);
});

test("Codes match confirms the pairing and reports the result", async () => {
  vi.mocked(p2pApi.confirmPairingCode).mockResolvedValue({ kind: "paired", peer_id: nearbyPeerId, device_name: "Laptop" });
  render(
    <DevicesPanel
      initialStatus={runningStatus({ outgoing_pairings: [{ peer_id: nearbyPeerId, code: "639210", locally_confirmed: false }] })}
    />
  );

  fireEvent.click(screen.getByRole("button", { name: "Codes match" }));

  expect(await screen.findByText("Paired with Laptop.")).toBeInTheDocument();
  expect(p2pApi.confirmPairingCode).toHaveBeenCalledWith(nearbyPeerId);
});

test("a confirmed pairing waits for the other device", () => {
  render(
    <DevicesPanel
      initialStatus={runningStatus({ outgoing_pairings: [{ peer_id: nearbyPeerId, code: "639210", locally_confirmed: true }] })}
    />
  );

  expect(screen.getByText("Waiting for the other device…")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Codes match" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Cancel pairing" })).toBeInTheDocument();
});

test("an incoming request can be accepted or rejected", async () => {
  vi.mocked(p2pApi.rejectPairingRequest).mockResolvedValue();
  vi.mocked(p2pApi.acceptPairingRequest).mockResolvedValue({ kind: "rejected", peer_id: nearbyPeerId });
  const withRequest = runningStatus({ incoming_requests: [{ peer_id: nearbyPeerId, device_name: "Phone", code: "580078" }] });
  vi.mocked(p2pApi.getP2pStatus).mockResolvedValue(withRequest);
  render(<DevicesPanel initialStatus={withRequest} />);

  const card = screen.getByRole("region", { name: "Pairing requests" });
  expect(within(card).getByText("Phone")).toBeInTheDocument();
  expect(within(card).getByText("Accept only if the other device shows the same code.")).toBeInTheDocument();
  fireEvent.click(within(card).getByRole("button", { name: "Accept" }));
  expect(await screen.findByText("The other device rejected the pairing.")).toBeInTheDocument();

  fireEvent.click(within(card).getByRole("button", { name: "Reject" }));
  await act(async () => {});
  expect(p2pApi.rejectPairingRequest).toHaveBeenCalledWith(nearbyPeerId);
});

test("removing a paired device asks first", async () => {
  vi.mocked(p2pApi.removeDevice).mockResolvedValue();
  render(
    <DevicesPanel
      initialStatus={runningStatus({ trusted_devices: [{ peer_id: pairedPeerId, device_name: "Laptop", paired_at_ms: Date.UTC(2026, 9, 28, 12) }] })}
    />
  );

  expect(screen.getByText("Paired 28 Oct 2026")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Remove Laptop" }));
  expect(screen.getByText("Laptop stops syncing with this server. Pair again to resume.")).toBeInTheDocument();
  expect(p2pApi.removeDevice).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "Remove" }));
  await act(async () => {});
  expect(p2pApi.removeDevice).toHaveBeenCalledWith(pairedPeerId);
});

test("a refused step is explained without the server's text", async () => {
  vi.mocked(p2pApi.cancelPairing).mockRejectedValue(new p2pApi.P2pRequestError("refused", 409));
  render(
    <DevicesPanel
      initialStatus={runningStatus({ outgoing_pairings: [{ peer_id: nearbyPeerId, code: "639210", locally_confirmed: false }] })}
    />
  );

  fireEvent.click(screen.getByRole("button", { name: "Cancel pairing" }));

  expect(await screen.findByText("The other device is no longer pairing. Try again.")).toBeInTheDocument();
});

test("the status refreshes every two seconds", async () => {
  vi.useFakeTimers();
  vi.mocked(p2pApi.getP2pStatus).mockResolvedValue(runningStatus({ nearby_devices: [{ peer_id: nearbyPeerId }] }));
  render(<DevicesPanel initialStatus={runningStatus()} />);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(2_000);
  });

  expect(screen.getByText("12D3Koo…abcdef")).toBeInTheDocument();
});
