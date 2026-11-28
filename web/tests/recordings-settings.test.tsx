// Verifies the web side of kept meeting recordings (#140): keeping is turned on only after the
// privacy warning, and wiping asks first. The meeting page's player is in recording-player.test.tsx.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { RecordingsSettings } from "@/components/recordings-settings";
import * as api from "@/lib/recordings-api";

vi.mock("@/lib/recordings-api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/recordings-api")),
  getRecordingsStatus: vi.fn(),
  setKeepRecordings: vi.fn(),
  wipeRecordings: vi.fn(),
  hasMeetingRecording: vi.fn()
}));

const status: api.RecordingsStatus = { keep: false, count: 2, bytes: 2 * 1048576 };

beforeEach(() => {
  vi.mocked(api.getRecordingsStatus).mockReset();
  vi.mocked(api.setKeepRecordings).mockReset();
  vi.mocked(api.wipeRecordings).mockReset();
  vi.mocked(api.hasMeetingRecording).mockReset();
});

test("keeping is turned on only after the warning", async () => {
  vi.mocked(api.setKeepRecordings).mockResolvedValue({ ...status, keep: true });
  render(<RecordingsSettings initial={status} />);
  expect(screen.getByText("2 recordings kept · 2.0 MB")).toBeInTheDocument();

  fireEvent.click(screen.getByLabelText(/Keep meeting recordings/));
  expect(screen.getByText(/consent/)).toBeInTheDocument();
  expect(api.setKeepRecordings).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Keep recordings" }));

  expect(await screen.findByRole("checkbox", { checked: true })).toBeInTheDocument();
  expect(api.setKeepRecordings).toHaveBeenCalledWith(true);
});

test("turning it off needs no warning", async () => {
  vi.mocked(api.setKeepRecordings).mockResolvedValue(status);
  render(<RecordingsSettings initial={{ ...status, keep: true }} />);
  fireEvent.click(screen.getByLabelText(/Keep meeting recordings/));
  expect(await screen.findByRole("checkbox", { checked: false })).toBeInTheDocument();
  expect(api.setKeepRecordings).toHaveBeenCalledWith(false);
});

test("wiping asks first, then deletes them all", async () => {
  vi.mocked(api.wipeRecordings).mockResolvedValue(2);
  vi.mocked(api.getRecordingsStatus).mockResolvedValue({ ...status, count: 0, bytes: 0 });
  render(<RecordingsSettings initial={status} />);

  fireEvent.click(screen.getByRole("button", { name: "Wipe all recordings" }));
  expect(screen.getByText("Wipe all recordings?")).toBeInTheDocument();
  expect(api.wipeRecordings).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Wipe all recordings" }));

  expect(await screen.findByText("2 recordings wiped.")).toBeInTheDocument();
  expect(screen.getByText("No recordings kept")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Wipe all recordings" })).toBeDisabled();
});
