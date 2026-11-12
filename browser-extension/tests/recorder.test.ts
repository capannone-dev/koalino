/**
 * Specifies one recording in the content script: what it sends to Koalino and when.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { MeetingRecorder, RecorderPorts, ownerParticipantId } from "../src/content/recorder";
import { HostMessage } from "../src/lib/messages";

/** A recorder with a controllable clock and the messages it sent. */
function recorderFixture() {
  const sent: HostMessage[] = [];
  let clock = 1_000_000;
  const ports: RecorderPorts = {
    send: (message) => sent.push(message),
    now: () => clock,
    root: document,
  };
  const recorder = new MeetingRecorder({ meetingId: "browser-teams-1", title: "Weekly sync", ownerName: "Ada" }, ports);
  return {
    sent,
    recorder,
    advance: (ms: number) => {
      clock += ms;
    },
  };
}

/** One second of audio at `level`. */
const second = (level: number) => new Int16Array(16_000).fill(level);

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("ownerParticipantId", () => {
  it("is the same for one owner however the name is spaced or cased", () => {
    expect(ownerParticipantId(" Ada  Lovelace ")).toBe(ownerParticipantId("ada lovelace"));
  });

  // The app keeps the first name it saw for a participant ID, so a fixed ID kept a wrong name
  // ("Your profile") from an earlier version for good, and two owners of one browser would merge.
  it("differs between owners", () => {
    expect(ownerParticipantId("Ada Lovelace")).not.toBe(ownerParticipantId("Grace Hopper"));
  });

  it("is the plain owner ID when the name is unknown", () => {
    expect(ownerParticipantId("Me")).toBe("browser-owner");
  });
});

describe("MeetingRecorder", () => {
  it("announces the meeting with the owner", () => {
    const { sent } = recorderFixture();
    expect(sent[0]).toEqual({
      type: "meeting",
      meetingId: "browser-teams-1",
      title: "Weekly sync",
      startTimeMs: 1_000_000,
      participants: [{ id: ownerParticipantId("Ada"), name: "Ada" }],
    });
  });

  it("sends ten-second chunks of each track, skipping silence", () => {
    const { sent, recorder, advance } = recorderFixture();
    for (let index = 0; index < 20; index += 1) {
      advance(1_000);
      recorder.addAudio("remote", second(500));
      recorder.addAudio("microphone", second(0));
    }
    const audio = sent.filter((message) => message.type === "audio");
    expect(audio).toHaveLength(2);
    expect(audio.every((message) => message.type === "audio" && message.track === "remote")).toBe(true);
    expect(audio.map((message) => (message.type === "audio" ? message.timestampMs : -1))).toEqual([0, 10_000]);
  });

  it("names the owner on microphone chunks", () => {
    const { sent, recorder, advance } = recorderFixture();
    for (let index = 0; index < 10; index += 1) {
      advance(1_000);
      recorder.addAudio("microphone", second(400));
    }
    const [chunk] = sent.filter((message) => message.type === "audio");
    expect(chunk?.type === "audio" && chunk.participant?.id).toBe(ownerParticipantId("Ada"));
  });

  it("restarts a track's timing after a gap", () => {
    const { sent, recorder, advance } = recorderFixture();
    advance(1_000);
    recorder.addAudio("remote", second(500));
    advance(30_000);
    for (let index = 0; index < 10; index += 1) {
      recorder.addAudio("remote", second(500));
      advance(1_000);
    }
    const stamps = sent.flatMap((message) => (message.type === "audio" ? [message.timestampMs] : []));
    expect(stamps).toEqual([0, 30_000]);
  });

  it("ends with the captions, the live transcript and the meeting end", () => {
    const { sent, recorder, advance } = recorderFixture();
    document.body.innerHTML = `
      <div data-tid="call-transcript-indicator"></div>
      <div><div><span>Alan</span></div><span data-tid="closed-caption-text">Hello there.</span></div>
      <div data-tid="transcript-chat-message-g/1"><span data-tid="transcript-item-author">Alan</span>
        <div data-tid="call-transcript-panel-message">Hello there.</div></div>`;
    advance(2_000);
    recorder.observePage({ captions: true });
    recorder.stop();
    recorder.stop();

    expect(recorder.sawTeamsTranscript()).toBe(true);
    const tail = sent.slice(1).map((message) => (message.type === "document" ? message.kind : message.type));
    expect(tail).toEqual(["teams_captions", "teams_live_transcript", "ended"]);
    const captions = sent[1];
    expect(captions?.type === "document" && captions.text).toContain("00:00:02.000 --> 00:00:02.000\n<v Alan>Hello there.</v>");
  });

  it("keeps a slide once the share settles, stamped when it settled", () => {
    const { sent, recorder, advance } = recorderFixture();
    const signature = new Array<number>(32 * 18).fill(200);
    expect(recorder.offerShareSignature(signature)).toBe(false);
    advance(1_000);
    expect(recorder.offerShareSignature(signature)).toBe(true);
    expect(recorder.offerShareSignature(signature)).toBe(false);
    advance(500);
    recorder.sendSlide("/9j/AA==");
    const slides = sent.filter((message) => message.type === "slide");
    expect(slides).toEqual([{ type: "slide", timestampMs: 1_000, image: "/9j/AA==" }]);
    expect(recorder.counts().slides).toBe(1);
  });

  it("sends no slide that was not asked for", () => {
    const { sent, recorder } = recorderFixture();
    recorder.sendSlide("/9j/AA==");
    expect(sent.filter((message) => message.type === "slide")).toHaveLength(0);
  });
});
