// Verifies the voice gallery assigns unknown-speaker clusters to typed names via the API client.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { VoiceGallery } from "@/components/voice-gallery";
import * as api from "@/lib/api";

vi.mock("@/lib/api", async () => ({ ...(await vi.importActual<typeof api>("@/lib/api")), deleteUnknownVoice: vi.fn(), deleteSavedPerson: vi.fn(), assignCluster: vi.fn(), assignVoiceGroup: vi.fn() }));

const clusters: api.LibraryCluster[] = [
  { cluster_id: "cluster-1", meeting_id: "meeting-1", total_duration_ms: 12_500, chunk_count: 3, created_at: "2026-10-27T09:15:00+00:00", meeting_title: "Project Sync", has_review_clip: true },
  { cluster_id: "cluster-2", meeting_id: "meeting-1", total_duration_ms: 700, chunk_count: 1, created_at: "2026-10-27T09:16:00+00:00", meeting_title: null, has_review_clip: false }
];

beforeEach(() => {
  vi.mocked(api.assignCluster).mockReset();
  vi.mocked(api.assignVoiceGroup).mockReset();
  vi.mocked(api.deleteUnknownVoice).mockReset();
});

test("renders one card per unresolved cluster", () => {
  render(<VoiceGallery clusters={clusters} people={[]} />);
  expect(screen.getByText("12.5 s · 3 chunks")).toBeInTheDocument();
  expect(screen.getByText("0.7 s · 1 chunk")).toBeInTheDocument();
  expect(screen.getAllByRole("button", { name: "Assign" })).toHaveLength(2);
});

test("Assign sends the typed name for that cluster and removes it", async () => {
  vi.mocked(api.assignCluster).mockResolvedValue({ cluster_id: "cluster-2", identity_id: "identity-1", identity_name: "Grace Hopper", identity_created: true, variants_stored: 1, segments_updated: 2 });
  render(<VoiceGallery clusters={clusters} people={[]} />);

  fireEvent.change(screen.getByLabelText("Name for unknown speaker 2"), { target: { value: "  Grace Hopper " } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[1]);

  expect(await screen.findByText("Assigned to Grace Hopper.")).toBeInTheDocument();
  expect(api.assignCluster).toHaveBeenCalledWith("cluster-2", "Grace Hopper");
  expect(screen.queryByText("0.7 s · 1 chunk")).not.toBeInTheDocument();
  expect(screen.getByText("12.5 s · 3 chunks")).toBeInTheDocument();
  expect(screen.getByText("Grace Hopper: 1 voice sample")).toBeInTheDocument();
});

test("Assign says when no voice could be saved", async () => {
  vi.mocked(api.assignCluster).mockResolvedValue({ cluster_id: "cluster-2", identity_id: "identity-1", identity_name: "Grace Hopper", identity_created: true, variants_stored: 0, segments_updated: 2 });
  render(<VoiceGallery clusters={clusters} people={[]} />);

  fireEvent.change(screen.getByLabelText("Name for unknown speaker 2"), { target: { value: "Grace Hopper" } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[1]);

  expect(await screen.findByText("Assigned to Grace Hopper. There was not enough clear speech to recognize this voice in future recordings.")).toBeInTheDocument();
});

test("a blank name is rejected without calling the API", async () => {
  render(<VoiceGallery clusters={clusters} people={[]} />);

  fireEvent.change(screen.getByLabelText("Name for unknown speaker 1"), { target: { value: "   " } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[0]);

  expect(await screen.findByText("Enter a name before assigning.")).toBeInTheDocument();
  expect(api.assignCluster).not.toHaveBeenCalled();
});

test("a rejected assignment keeps the cluster and reports the failure", async () => {
  vi.mocked(api.assignCluster).mockRejectedValue(new Error("Cluster assignment failed: 409"));
  render(<VoiceGallery clusters={clusters} people={[]} />);

  fireEvent.change(screen.getByLabelText("Name for unknown speaker 1"), { target: { value: "Ada" } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[0]);

  expect(await screen.findByText("Unable to assign the speaker cluster.")).toBeInTheDocument();
  expect(screen.getByText("12.5 s · 3 chunks")).toBeInTheDocument();
});

test("shows meeting titles, playable clips, and saved people offered as names", () => {
  const { container } = render(<VoiceGallery clusters={clusters} people={[{ identity_id: "identity-1", display_name: "Ada Lovelace", voice_variant_count: 3 }]} />);

  expect(screen.getByText("Project Sync")).toBeInTheDocument();
  expect(screen.getByText("Untitled meeting")).toBeInTheDocument();
  expect(screen.getByLabelText("Listen to unknown speaker 1")).toHaveAttribute("src", expect.stringContaining("/api/clusters/cluster-1/review-clip"));
  expect(screen.queryByLabelText("Listen to unknown speaker 2")).not.toBeInTheDocument();
  expect(screen.getByText("Ada Lovelace: 3 voice samples")).toBeInTheDocument();
  expect(container.querySelector('datalist option[value="Ada Lovelace"]')).not.toBeNull();
});

test("explains when there are no saved people", () => {
  render(<VoiceGallery clusters={[]} people={[]} />);

  expect(screen.getByText("No unknown speakers to review.")).toBeInTheDocument();
  expect(screen.getByText("No saved people yet. Name a voice to add someone.")).toBeInTheDocument();
});

test("deleting an unknown voice asks first, then removes its card", async () => {
  const deleted = vi.mocked(api.deleteUnknownVoice).mockResolvedValue();
  render(<VoiceGallery clusters={clusters} people={[]} />);

  fireEvent.click(screen.getByRole("button", { name: "Delete Unknown speaker 1" }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(deleted).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "Delete Unknown speaker 1" }));
  fireEvent.click(screen.getByRole("button", { name: "Delete" }));

  expect(await screen.findByText("Voice deleted.")).toBeInTheDocument();
  expect(deleted).toHaveBeenCalledWith("cluster-1");
  expect(screen.queryByText("12.5 s · 3 chunks")).not.toBeInTheDocument();
  deleted.mockReset();
});

test("a saved person can be deleted, and a refusal keeps them and explains", async () => {
  const deleted = vi.mocked(api.deleteSavedPerson).mockRejectedValueOnce(new api.DeletionError("failed")).mockResolvedValue();
  const ada = { identity_id: "identity-1", display_name: "Ada", voice_variant_count: 2 };
  render(<VoiceGallery clusters={[]} people={[ada]} />);

  fireEvent.click(screen.getByRole("button", { name: "Delete Ada" }));
  expect(screen.getByRole("alertdialog", { name: "Delete Ada?" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Delete" }));
  expect(await screen.findByText(/Couldn't delete this/)).toBeInTheDocument();
  expect(screen.getByText(/Ada: 2 voice/)).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Delete" }));
  expect(await screen.findByText("Ada deleted.")).toBeInTheDocument();
  expect(screen.queryByText(/Ada: 2 voice/)).not.toBeInTheDocument();
  deleted.mockReset();
});

/** One person heard in two meetings, and someone else (#82). */
const lunch: api.LibraryCluster[] = [
  { cluster_id: "part-1", meeting_id: "meeting-1", total_duration_ms: 105_000, chunk_count: 1, created_at: "2026-11-02T12:00:00+00:00", meeting_title: "Lunch", has_review_clip: true, voice_group_id: "part-1" },
  { cluster_id: "other-1", meeting_id: "meeting-1", total_duration_ms: 22_000, chunk_count: 1, created_at: "2026-11-02T12:01:00+00:00", meeting_title: "Lunch", has_review_clip: false, voice_group_id: "other-1" },
  { cluster_id: "part-2", meeting_id: "meeting-2", total_duration_ms: 29_000, chunk_count: 1, created_at: "2026-11-02T13:00:00+00:00", meeting_title: "Standup", has_review_clip: true, voice_group_id: "part-1" }
];

test("voices of one person share one card that says what it covers", () => {
  render(<VoiceGallery clusters={lunch} people={[]} />);

  expect(screen.getByText("Unknown speaker 1")).toBeInTheDocument();
  expect(screen.getByText("Unknown speaker 2")).toBeInTheDocument();
  expect(screen.queryByText("Unknown speaker 3")).not.toBeInTheDocument();
  expect(screen.getByText("134.0 s · 2 parts in 2 meetings")).toBeInTheDocument();
  expect(screen.getByText("Lunch, Standup")).toBeInTheDocument();
  expect(screen.getAllByRole("button", { name: "Assign" })).toHaveLength(2);
});

test("a group is named at once, without the parts left out", async () => {
  vi.mocked(api.assignVoiceGroup).mockResolvedValue({ identity_id: "identity-1", identity_name: "Marco", identity_created: true, assigned_cluster_ids: ["part-1"], skipped_cluster_count: 0, variants_stored: 1, segments_updated: 4 });
  render(<VoiceGallery clusters={lunch} people={[]} />);

  fireEvent.click(screen.getByLabelText("Include Standup · 29.0 s"));
  fireEvent.change(screen.getByLabelText("Name for unknown speaker 1"), { target: { value: " Marco " } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[0]);

  expect(await screen.findByText("Assigned to Marco.")).toBeInTheDocument();
  expect(api.assignVoiceGroup).toHaveBeenCalledWith(["part-1"], "Marco");
  expect(api.assignCluster).not.toHaveBeenCalled();
  expect(screen.getByText("29.0 s · 1 chunk")).toBeInTheDocument();
});

test("naming a whole group says how many voices it named", async () => {
  vi.mocked(api.assignVoiceGroup).mockResolvedValue({ identity_id: "identity-1", identity_name: "Marco", identity_created: true, assigned_cluster_ids: ["part-1", "part-2"], skipped_cluster_count: 0, variants_stored: 2, segments_updated: 4 });
  render(<VoiceGallery clusters={lunch} people={[]} />);

  fireEvent.change(screen.getByLabelText("Name for unknown speaker 1"), { target: { value: "Marco" } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[0]);

  expect(await screen.findByText("Assigned 2 voices to Marco.")).toBeInTheDocument();
  expect(api.assignVoiceGroup).toHaveBeenCalledWith(["part-1", "part-2"], "Marco");
  expect(screen.queryByText("Unknown speaker 2")).not.toBeInTheDocument();
  expect(screen.getByText("Marco: 2 voice samples")).toBeInTheDocument();
});

test("with every part left out, nothing is named", async () => {
  render(<VoiceGallery clusters={lunch} people={[]} />);

  fireEvent.click(screen.getByLabelText("Include Lunch · 105.0 s"));
  fireEvent.click(screen.getByLabelText("Include Standup · 29.0 s"));
  fireEvent.change(screen.getByLabelText("Name for unknown speaker 1"), { target: { value: "Marco" } });
  fireEvent.click(screen.getAllByRole("button", { name: "Assign" })[0]);

  expect(await screen.findByText("Choose at least one voice to name.")).toBeInTheDocument();
  expect(api.assignVoiceGroup).not.toHaveBeenCalled();
});

test("deleting a group deletes every part", async () => {
  vi.mocked(api.deleteUnknownVoice).mockResolvedValue();
  render(<VoiceGallery clusters={lunch} people={[]} />);

  fireEvent.click(screen.getByRole("button", { name: "Delete Unknown speaker 1" }));
  fireEvent.click(await screen.findByRole("button", { name: "Delete" }));

  expect(await screen.findByText("Voice deleted.")).toBeInTheDocument();
  expect(vi.mocked(api.deleteUnknownVoice).mock.calls).toEqual([["part-1"], ["part-2"]]);
  expect(screen.queryByText("134.0 s · 2 parts in 2 meetings")).not.toBeInTheDocument();
});
