// Verifies the web Settings page's speech retention window (#93): the saved window shown in days,
// choosing another, and the reasons a change fails.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { RetentionSettings } from "@/components/retention-settings";
import * as api from "@/lib/api";

vi.mock("@/lib/api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/api")),
  setRetention: vi.fn()
}));

const threeWeeks: api.SpeechRetention = { days: 21, min_days: 1, max_days: 60 };

beforeEach(() => {
  vi.mocked(api.setRetention).mockReset();
});

test("shows the saved window and offers the bounds", () => {
  render(<RetentionSettings initial={{ ...threeWeeks, days: 9 }} />);

  const menu = screen.getByLabelText("Keep speech for combining");
  expect(menu).toHaveValue("9");
  const values = Array.from((menu as HTMLSelectElement).options).map((option) => option.value);
  expect(values[0]).toBe("1");
  expect(values.at(-1)).toBe("60");
});

test("choosing a window saves it and says so", async () => {
  vi.mocked(api.setRetention).mockResolvedValue({ ...threeWeeks, days: 7 });
  render(<RetentionSettings initial={threeWeeks} />);

  fireEvent.change(screen.getByLabelText("Keep speech for combining"), { target: { value: "7" } });

  expect(await screen.findByText("Saved. Speech now waits up to 7 days to be combined.")).toBeInTheDocument();
  expect(api.setRetention).toHaveBeenCalledWith(7);
  expect(screen.getByLabelText("Keep speech for combining")).toHaveValue("7");
});

test("a refused window goes back to the saved one", async () => {
  vi.mocked(api.setRetention).mockRejectedValue(new api.RetentionError("invalid_window", 400));
  render(<RetentionSettings initial={threeWeeks} />);

  fireEvent.change(screen.getByLabelText("Keep speech for combining"), { target: { value: "7" } });

  expect(await screen.findByText(/between 1 and 60 days/)).toBeInTheDocument();
  expect(screen.getByLabelText("Keep speech for combining")).toHaveValue("21");
});

test("without the server the window cannot be changed", () => {
  render(<RetentionSettings initial={null} />);

  expect(screen.getByLabelText("Keep speech for combining")).toBeDisabled();
  expect(screen.getByText(/Can't read how long speech is kept/)).toBeInTheDocument();
});
