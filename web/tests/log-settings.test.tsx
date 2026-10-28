// Verifies the web Settings page's log controls (Issue #46): choosing a level, its messages, and
// downloading the logs.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { LogSettings } from "@/components/log-settings";
import * as api from "@/lib/api";

vi.mock("@/lib/api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/api")),
  setLogLevel: vi.fn(),
  fetchLogExport: vi.fn()
}));

const saveFile = vi.fn();

beforeEach(() => {
  vi.mocked(api.setLogLevel).mockReset();
  vi.mocked(api.fetchLogExport).mockReset();
  saveFile.mockReset();
});

test("shows the level in effect", () => {
  render(<LogSettings initialLevel="info" saveFile={saveFile} />);

  expect(screen.getByLabelText("Log level")).toHaveValue("info");
  expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["Error", "Warning", "Info (default)", "Debug", "Trace"]);
});

test("a directive the page does not offer is shown as custom", () => {
  render(<LogSettings initialLevel="note_taker=trace,info" saveFile={saveFile} />);

  expect(screen.getByLabelText("Log level")).toHaveValue("note_taker=trace,info");
  expect(screen.getByRole("option", { name: "Custom (note_taker=trace,info)" })).toBeInTheDocument();
});

test("choosing a level applies it", async () => {
  vi.mocked(api.setLogLevel).mockResolvedValue("debug");
  render(<LogSettings initialLevel="info" saveFile={saveFile} />);

  fireEvent.change(screen.getByLabelText("Log level"), { target: { value: "debug" } });

  expect(await screen.findByText("The server now logs at Debug until it restarts.")).toBeInTheDocument();
  expect(api.setLogLevel).toHaveBeenCalledWith("debug");
  expect(screen.getByLabelText("Log level")).toHaveValue("debug");
});

test("a refused change keeps the old level and explains", async () => {
  vi.mocked(api.setLogLevel).mockRejectedValue(new api.LogSettingsError("network", null));
  render(<LogSettings initialLevel="info" saveFile={saveFile} />);

  fireEvent.change(screen.getByLabelText("Log level"), { target: { value: "trace" } });

  expect(await screen.findByText("Can't reach the note-taking server. Check that it is running.")).toBeInTheDocument();
  expect(screen.getByLabelText("Log level")).toHaveValue("info");
});

test("without the server the level cannot be changed", () => {
  render(<LogSettings initialLevel={null} saveFile={saveFile} />);

  expect(screen.getByLabelText("Log level")).toBeDisabled();
  expect(screen.getByText("Can't read the log level. Check that the note-taking server is running.")).toBeInTheDocument();
});

test("Export logs downloads the file", async () => {
  const blob = new Blob(["log\n"]);
  vi.mocked(api.fetchLogExport).mockResolvedValue({ fileName: "note_taker_logs_20261028-090507.jsonl", blob });
  render(<LogSettings initialLevel="info" saveFile={saveFile} />);

  fireEvent.click(screen.getByRole("button", { name: "Export logs" }));

  expect(await screen.findByText("Downloaded note_taker_logs_20261028-090507.jsonl.")).toBeInTheDocument();
  expect(saveFile).toHaveBeenCalledWith(blob, "note_taker_logs_20261028-090507.jsonl");
});

test("Export logs explains when there are no logs yet", async () => {
  vi.mocked(api.fetchLogExport).mockRejectedValue(new api.LogSettingsError("no_logs", 404));
  render(<LogSettings initialLevel="info" saveFile={saveFile} />);

  fireEvent.click(screen.getByRole("button", { name: "Export logs" }));

  expect(await screen.findByText("There are no logs to export yet.")).toBeInTheDocument();
  expect(saveFile).not.toHaveBeenCalled();
});
