// Verifies a combined meeting's page names its devices and splits it after asking (#88).

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingRecordedOn } from "@/components/meeting-recorded-on";
import * as api from "@/lib/api";

const routerRefresh = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), refresh: routerRefresh }) }));
vi.mock("@/lib/api", async () => ({ ...(await vi.importActual<typeof api>("@/lib/api")), splitMeeting: vi.fn() }));

const combined = {
  meeting_id: "meeting-laptop",
  recorded_on: [
    { device_id: "device-laptop", name: "Laptop", this_device: true },
    { device_id: "device-phone", name: "Pixel", this_device: false }
  ],
  combined_recordings: ["meeting-phone"]
};

beforeEach(() => {
  vi.mocked(api.splitMeeting).mockReset();
  routerRefresh.mockReset();
});

test("a meeting from one device shows nothing", () => {
  const { container } = render(<MeetingRecordedOn meeting={{ meeting_id: "m", recorded_on: [], combined_recordings: [] }} />);
  expect(container).toBeEmptyDOMElement();
});

test("splitting asks first, then splits and refreshes the page", async () => {
  vi.mocked(api.splitMeeting).mockResolvedValue(["meeting-phone"]);
  render(<MeetingRecordedOn meeting={combined} />);

  expect(screen.getByText("Recorded on Laptop + Pixel")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Split" }));
  expect(api.splitMeeting).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Split for good" }));

  await waitFor(() => expect(api.splitMeeting).toHaveBeenCalledWith("meeting-laptop"));
  await waitFor(() => expect(routerRefresh).toHaveBeenCalled());
  expect(screen.getByRole("status")).toHaveTextContent("Split: meeting-phone is listed on its own again");
});

test("a refused split says so and keeps the button", async () => {
  vi.mocked(api.splitMeeting).mockRejectedValue(new Error("Split failed (500)"));
  render(<MeetingRecordedOn meeting={combined} />);

  fireEvent.click(screen.getByRole("button", { name: "Split" }));
  fireEvent.click(screen.getByRole("button", { name: "Split for good" }));

  await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Unable to split this meeting"));
  expect(routerRefresh).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Split" })).toBeInTheDocument();
});
