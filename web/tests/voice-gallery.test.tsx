// Verifies the voice gallery assigns unknown-speaker clusters to typed names via the API client.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { VoiceGallery } from "@/components/voice-gallery";
import * as api from "@/lib/api";

vi.mock("@/lib/api", async () => ({ ...(await vi.importActual<typeof api>("@/lib/api")), assignCluster: vi.fn() }));

const clusters: api.UnknownCluster[] = [
  { cluster_id: "cluster-1", meeting_id: "meeting-1", total_duration_ms: 12_500, chunk_count: 3, created_at: "2026-10-27T09:15:00+00:00" },
  { cluster_id: "cluster-2", meeting_id: "meeting-1", total_duration_ms: 700, chunk_count: 1, created_at: "2026-10-27T09:16:00+00:00" }
];

beforeEach(() => {
  vi.mocked(api.assignCluster).mockReset();
});

test("renders one card per unresolved cluster", () => {
  render(<VoiceGallery clusters={clusters} />);
  expect(screen.getByText("12.5 s · 3 chunks")).toBeInTheDocument();
  expect(screen.getByText("0.7 s · 1 chunk")).toBeInTheDocument();
  expect(screen.getAllByRole("button", { name: "Assign" })).toHaveLength(2);
});

test("Assign sends the typed name for that cluster and removes it", async () => {
  vi.mocked(api.assignCluster).mockResolvedValue({ cluster_id: "cluster-2", identity_id: "identity-1", identity_name: "Grace Hopper", identity_created: true, variants_stored: 1, segments_updated: 2 });
  render(<VoiceGallery clusters={clusters} />);

  fireEvent.change(screen.getByLabelText("Name for unknown speaker 2"), { target: { value: "  Grace Hopper " } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[1]);

  expect(await screen.findByText("Assigned to Grace Hopper.")).toBeInTheDocument();
  expect(api.assignCluster).toHaveBeenCalledWith("cluster-2", "Grace Hopper");
  expect(screen.queryByText("0.7 s · 1 chunk")).not.toBeInTheDocument();
  expect(screen.getByText("12.5 s · 3 chunks")).toBeInTheDocument();
});

test("a blank name is rejected without calling the API", async () => {
  render(<VoiceGallery clusters={clusters} />);

  fireEvent.change(screen.getByLabelText("Name for unknown speaker 1"), { target: { value: "   " } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[0]);

  expect(await screen.findByText("Enter a name before assigning.")).toBeInTheDocument();
  expect(api.assignCluster).not.toHaveBeenCalled();
});

test("a rejected assignment keeps the cluster and reports the failure", async () => {
  vi.mocked(api.assignCluster).mockRejectedValue(new Error("Cluster assignment failed: 409"));
  render(<VoiceGallery clusters={clusters} />);

  fireEvent.change(screen.getByLabelText("Name for unknown speaker 1"), { target: { value: "Ada" } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[0]);

  expect(await screen.findByText("Unable to assign the speaker cluster.")).toBeInTheDocument();
  expect(screen.getByText("12.5 s · 3 chunks")).toBeInTheDocument();
});

test("shows an empty state when nothing needs review", () => {
  render(<VoiceGallery clusters={[]} />);
  expect(screen.getByText("No unknown speakers to review.")).toBeInTheDocument();
});
