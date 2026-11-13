// Verifies the web Settings page's after-meeting pass (#87): the switch, the model menu of models on
// the server's computer, and the reasons a change fails.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { PassSettings } from "@/components/pass-settings";
import * as passApi from "@/lib/pass-api";

vi.mock("@/lib/pass-api", async () => ({
  ...(await vi.importActual<typeof passApi>("@/lib/pass-api")),
  setAfterMeetingPass: vi.fn()
}));

const onWithTiny: passApi.AfterMeetingPass = {
  enabled: true,
  model: null,
  models: [{ file_name: "ggml-tiny.bin", name: "tiny" }]
};

beforeEach(() => {
  vi.mocked(passApi.setAfterMeetingPass).mockReset();
});

test("the pass is on with the recording model, and a model here can be chosen", async () => {
  vi.mocked(passApi.setAfterMeetingPass).mockResolvedValue({ ...onWithTiny, model: "ggml-tiny.bin" });
  render(<PassSettings initial={onWithTiny} />);

  expect(screen.getByLabelText("Improve the transcript after the meeting")).toBeChecked();
  const menu = screen.getByLabelText("Model for this pass");
  expect(menu).toHaveValue("");
  fireEvent.change(menu, { target: { value: "ggml-tiny.bin" } });

  expect(await screen.findByText("Saved. Meetings recorded from now on use this.")).toBeInTheDocument();
  expect(passApi.setAfterMeetingPass).toHaveBeenCalledWith({ model: "ggml-tiny.bin" });
  expect(screen.getByLabelText("Model for this pass")).toHaveValue("ggml-tiny.bin");
});

test("turning the pass off hides the model menu", async () => {
  vi.mocked(passApi.setAfterMeetingPass).mockResolvedValue({ ...onWithTiny, enabled: false });
  render(<PassSettings initial={onWithTiny} />);

  fireEvent.click(screen.getByLabelText("Improve the transcript after the meeting"));

  expect(await screen.findByText("Saved. Meetings recorded from now on use this.")).toBeInTheDocument();
  expect(passApi.setAfterMeetingPass).toHaveBeenCalledWith({ enabled: false });
  expect(screen.queryByLabelText("Model for this pass")).not.toBeInTheDocument();
});

test("a refused model says why, and an unreadable setting says so", async () => {
  vi.mocked(passApi.setAfterMeetingPass).mockRejectedValue(new passApi.PassError("invalid_model", 400));
  const { unmount } = render(<PassSettings initial={onWithTiny} />);

  fireEvent.change(screen.getByLabelText("Model for this pass"), { target: { value: "ggml-tiny.bin" } });

  expect(await screen.findByText("That model is not on the server's computer. Download it in the app first.")).toBeInTheDocument();
  unmount();
  render(<PassSettings initial={null} />);
  expect(screen.getByText("Can't read the after-meeting pass. Check that the note-taking server is running.")).toBeInTheDocument();
  expect(screen.getByLabelText("Improve the transcript after the meeting")).toBeDisabled();
});
