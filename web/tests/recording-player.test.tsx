// Verifies the meeting page's recording player (#197): it shows only for a kept recording, loads
// the audio only when played, plays a passage from its start, marks the passage being heard,
// changes speed, and stops following when the owner scrolls by hand. It also downloads the
// recording under the meeting's title (#199).

import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { MeetingRecordingPlayer, formatPlaybackTime } from "@/components/meeting-recording-player";
import { MeetingTranscript } from "@/components/meeting-transcript";
import { RecordingPlaybackProvider, passageStartAt, recordingFileName } from "@/lib/recording-playback";
import * as api from "@/lib/recordings-api";

vi.mock("@/lib/recordings-api", async () => ({
  ...(await vi.importActual<typeof api>("@/lib/recordings-api")),
  hasMeetingRecording: vi.fn()
}));

const segment = (start: number, speaker: string, text: string) => ({
  start_time_ms: start,
  end_time_ms: start + 4_000,
  speaker,
  text,
  overlapping_speech: false,
  uncertain_speaker: false,
  uncertain_words: []
});

const segments = [segment(0, "Alice", "Hello team."), segment(61_000, "Bob", "We ship on Friday.")];

let play: ReturnType<typeof vi.fn>;

beforeEach(() => {
  vi.mocked(api.hasMeetingRecording).mockReset();
  // jsdom has no media playback: playing fires the element's own play event.
  play = vi.fn(function (this: HTMLMediaElement) {
    Object.defineProperty(this, "paused", { configurable: true, value: false });
    this.dispatchEvent(new Event("play"));
    return Promise.resolve();
  });
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(play);
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(function (this: HTMLMediaElement) {
    Object.defineProperty(this, "paused", { configurable: true, value: true });
    this.dispatchEvent(new Event("pause"));
  });
  Element.prototype.scrollIntoView = vi.fn();
});

function renderPage() {
  return render(
    <RecordingPlaybackProvider meetingId="m 1">
      <MeetingTranscript transcript={null} segments={segments} />
      <MeetingRecordingPlayer title="Project sync" />
    </RecordingPlaybackProvider>
  );
}

test("a meeting without a kept recording shows no player", async () => {
  vi.mocked(api.hasMeetingRecording).mockResolvedValue(false);
  const { container } = renderPage();
  await act(async () => undefined);
  expect(container.querySelector("audio")).toBeNull();
  expect(screen.queryByRole("region", { name: "Recording player" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Play from 1:01" })).not.toBeInTheDocument();
});

test("the audio loads only when played, and a passage plays from its start", async () => {
  vi.mocked(api.hasMeetingRecording).mockResolvedValue(true);
  const { container } = renderPage();
  expect(await screen.findByRole("region", { name: "Recording player" })).toBeInTheDocument();
  const audio = container.querySelector("audio") as HTMLAudioElement;
  expect(audio.getAttribute("preload")).toBe("none");
  expect(audio.getAttribute("src")).toMatch(/\/api\/meetings\/m%201\/recording$/);

  fireEvent.click(screen.getByRole("button", { name: "Play from 1:01" }));
  expect(audio.currentTime).toBe(61);
  expect(play).toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
  expect(screen.getByText("Bob · 1:01").closest("li")).toHaveAttribute("aria-current", "true");

  act(() => {
    Object.defineProperty(audio, "currentTime", { configurable: true, value: 2 });
    fireEvent.timeUpdate(audio);
  });
  expect(screen.getByText("Alice · 0:00").closest("li")).toHaveAttribute("aria-current", "true");
  expect(screen.getByText("Bob · 1:01").closest("li")).not.toHaveAttribute("aria-current");
  expect(Element.prototype.scrollIntoView).toHaveBeenCalled();
});

test("Space plays and pauses, L and J skip 10 seconds, not while typing (#215)", async () => {
  vi.mocked(api.hasMeetingRecording).mockResolvedValue(true);
  const { container } = renderPage();
  await screen.findByRole("region", { name: "Recording player" });
  const audio = container.querySelector("audio") as HTMLAudioElement;

  fireEvent.keyDown(document.body, { key: " " });
  expect(play).toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
  act(() => {
    Object.defineProperty(audio, "duration", { configurable: true, value: 65 });
    fireEvent.durationChange(audio);
  });
  fireEvent.keyDown(document.body, { key: "l" });
  expect(audio.currentTime).toBe(10);
  fireEvent.keyDown(document.body, { key: "j" });
  expect(audio.currentTime).toBe(0);

  const field = document.createElement("input");
  document.body.append(field);
  fireEvent.keyDown(field, { key: "l" });
  expect(audio.currentTime).toBe(0);
  field.remove();
});

test("the speed changes the audio's rate", async () => {
  vi.mocked(api.hasMeetingRecording).mockResolvedValue(true);
  const { container } = renderPage();
  fireEvent.change(await screen.findByTitle("Playback speed"), { target: { value: "1.5" } });
  expect((container.querySelector("audio") as HTMLAudioElement).playbackRate).toBe(1.5);
});

test("scrolling by hand while playing stops following until asked", async () => {
  vi.mocked(api.hasMeetingRecording).mockResolvedValue(true);
  renderPage();
  fireEvent.click(await screen.findByRole("button", { name: "Play" }));
  expect(screen.queryByRole("button", { name: "Follow along in the transcript" })).not.toBeInTheDocument();
  fireEvent.wheel(window);
  fireEvent.click(screen.getByRole("button", { name: "Follow along in the transcript" }));
  expect(screen.queryByRole("button", { name: "Follow along in the transcript" })).not.toBeInTheDocument();
});

test("times and passages are read as on the transcript", () => {
  expect(formatPlaybackTime(61_000)).toBe("1:01");
  expect(formatPlaybackTime(3_725_000)).toBe("1:02:05");
  expect(passageStartAt([0, 5_000, 12_000], 13_000)).toBe(12_000);
  expect(passageStartAt([5_000], 1_000)).toBeNull();
});

test("the recording downloads under the meeting's title (#199, #200)", async () => {
  vi.mocked(api.hasMeetingRecording).mockResolvedValue(true);
  renderPage();
  const link = await screen.findByRole("link", { name: "Download the recording" });
  expect(link.getAttribute("href")).toMatch(/\/api\/meetings\/m%201\/recording$/);
  expect(link.getAttribute("download")).toBe("Project sync.wav");
  expect(recordingFileName("Q3: plan/review")).toBe("Q3_ plan_review.wav");
  expect(recordingFileName(null)).toBe("meeting.wav");
});
