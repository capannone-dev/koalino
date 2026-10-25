// Verifies voice assignment posts the selected opaque identifiers to the API client boundary.

import { fireEvent, render, screen } from "@testing-library/react";
import { vi } from "vitest";
import { VoiceGallery } from "@/components/voice-gallery";
import * as api from "@/lib/api";

vi.mock("@/lib/api", async () => ({ ...(await vi.importActual<typeof api>("@/lib/api")), assignVoiceClip: vi.fn().mockResolvedValue(undefined) }));

test("assigns an unassigned clip to the selected identity", async () => {
  render(<VoiceGallery gallery={{ identities: [{ identity_id: "identity-1", display_name: "Ada" }], unassigned_clips: [{ clip_id: "clip-1", meeting_id: "meeting-1", start_time_ms: 0, end_time_ms: 100 }] }} />);
  fireEvent.click(screen.getByRole("button", { name: "Assign clip" }));
  expect(await screen.findByText("Voice clip assigned.")).toBeInTheDocument();
  expect(api.assignVoiceClip).toHaveBeenCalledWith("clip-1", "identity-1");
});
