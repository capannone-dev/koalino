// Verifies the web Settings switch for keeping online-meeting participants' voices (#103): off
// until switched on, saved through the daemon, and back to the saved state when saving fails.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingVoicesSettings } from "@/components/meeting-voices-settings";
import * as api from "@/lib/meeting-voices-api";

vi.mock("@/lib/meeting-voices-api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/meeting-voices-api")),
  setMeetingVoices: vi.fn()
}));

const label = "Keep meeting participants' voices";

beforeEach(() => {
  vi.mocked(api.setMeetingVoices).mockReset();
});

test("shows the saved choice and explains rooms are never kept", () => {
  render(<MeetingVoicesSettings initial={{ keep: false }} />);

  expect(screen.getByRole("switch", { name: label })).not.toBeChecked();
  expect(screen.getByText(/meeting room/)).toBeInTheDocument();
});

test("switching on saves it and says so", async () => {
  vi.mocked(api.setMeetingVoices).mockResolvedValue({ keep: true });
  render(<MeetingVoicesSettings initial={{ keep: false }} />);

  fireEvent.click(screen.getByRole("switch", { name: label }));

  expect(await screen.findByText(/Saved\. Participants' voices will be kept/)).toBeInTheDocument();
  expect(api.setMeetingVoices).toHaveBeenCalledWith(true);
  expect(screen.getByRole("switch", { name: label })).toBeChecked();
});

test("a failed save goes back to the saved choice", async () => {
  vi.mocked(api.setMeetingVoices).mockRejectedValue(new api.MeetingVoicesError("server", 500));
  render(<MeetingVoicesSettings initial={{ keep: false }} />);

  fireEvent.click(screen.getByRole("switch", { name: label }));

  expect(await screen.findByText(/Something went wrong/)).toBeInTheDocument();
  expect(screen.getByRole("switch", { name: label })).not.toBeChecked();
});

test("without the server the switch cannot be changed", () => {
  render(<MeetingVoicesSettings initial={null} />);

  expect(screen.getByRole("switch", { name: label })).toBeDisabled();
  expect(screen.getByText(/Can't read whether meeting voices are kept/)).toBeInTheDocument();
});
