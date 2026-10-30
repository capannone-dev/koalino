// Specifies the transcription choice on the Settings page: it offers each device, applies a choice
// through the daemon, and keeps the old choice when refused.

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TranscriptionRouteSettings } from "@/components/transcription-route-settings";

const devices = [{ peerId: "12D3KooWLaptop", name: "Laptop" }];

afterEach(() => vi.unstubAllGlobals());

describe("TranscriptionRouteSettings", () => {
  it("offers this device, the best device and paired devices", () => {
    render(<TranscriptionRouteSettings initialRoute="local" devices={devices} />);

    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
      "This device",
      "Best available",
      "Laptop"
    ]);
  });

  it("applies a choice through the daemon", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);
    render(<TranscriptionRouteSettings initialRoute="local" devices={devices} />);

    fireEvent.change(screen.getByLabelText("Transcribe on"), { target: { value: "12D3KooWLaptop" } });

    await waitFor(() => expect(screen.getByLabelText("Transcribe on")).toHaveProperty("value", "12D3KooWLaptop"));
    const [url, init] = fetchMock.mock.calls[0];
    expect(String(url)).toContain("/api/p2p/transcription-route");
    expect(init.method).toBe("PUT");
    expect(JSON.parse(init.body)).toEqual({ route: "12D3KooWLaptop" });
  });

  it("keeps the old choice when refused", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 409 })));
    render(<TranscriptionRouteSettings initialRoute="auto" devices={devices} />);

    fireEvent.change(screen.getByLabelText("Transcribe on"), { target: { value: "12D3KooWLaptop" } });

    await screen.findByText(/could not be changed/);
    expect(screen.getByLabelText("Transcribe on")).toHaveProperty("value", "auto");
  });
});
