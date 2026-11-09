/**
 * Specifies how the content script reads a Teams call from the page, against markup modelled on
 * what Teams showed in the #102 probes (work and personal accounts).
 */
import { beforeEach, describe, expect, it } from "vitest";
import {
  findRecapCards,
  isInCall,
  isTeamsTranscriptRunning,
  meetingTitleFromDocumentTitle,
  parseRecapTimeRange,
  readCaptions,
  readTranscriptPanel,
} from "../src/lib/teamsDom";

beforeEach(() => {
  document.body.innerHTML = "";
});

describe("call state", () => {
  it("is in a call while the hang-up button is shown", () => {
    expect(isInCall(document)).toBe(false);
    document.body.innerHTML = `<button data-tid="hangup-main-btn">Leave</button>`;
    expect(isInCall(document)).toBe(true);
  });

  it("sees Teams transcription running, whoever started it", () => {
    expect(isTeamsTranscriptRunning(document)).toBe(false);
    document.body.innerHTML = `<div data-tid="call-transcript-indicator"></div>`;
    expect(isTeamsTranscriptRunning(document)).toBe(true);
  });

  it("takes the meeting title from the tab title", () => {
    expect(meetingTitleFromDocumentTitle("(3) Calendar | Weekly sync | Microsoft Teams")).toBe("Weekly sync");
    expect(meetingTitleFromDocumentTitle("Meeting with Ada | Microsoft Teams")).toBe("Meeting with Ada");
    expect(meetingTitleFromDocumentTitle("Microsoft Teams")).toBe("");
  });
});

describe("readCaptions", () => {
  it("reads work-account captions with the speaker's Teams ID", () => {
    document.body.innerHTML = `
      <div data-tid="closed-caption-v2-virtual-list-content">
        <div data-tid="closed-captions-v2-items-renderer" data-person-mri="8:orgid:ada">
          <div><span>Ada Lovelace</span></div>
          <span data-tid="closed-caption-text">Good morning</span>
        </div>
        <div data-tid="closed-captions-v2-items-renderer" data-person-mri="8:orgid:alan">
          <div><span>Alan Turing</span></div>
          <span data-tid="closed-caption-text">Hello</span>
        </div>
      </div>`;
    const captions = readCaptions(document);
    expect(captions.map(({ speaker, speakerId, text }) => ({ speaker, speakerId, text }))).toEqual([
      { speaker: "Ada Lovelace", speakerId: "8:orgid:ada", text: "Good morning" },
      { speaker: "Alan Turing", speakerId: "8:orgid:alan", text: "Hello" },
    ]);
  });

  it("reads personal-account captions with only a name", () => {
    document.body.innerHTML = `
      <div data-tid="closed-caption-v2-virtual-list-content">
        <div class="item"><img alt="" src="avatar.png"><div><span>Grace Hopper</span></div>
          <div><span data-tid="closed-caption-text">Any point forward</span></div></div>
      </div>`;
    const [caption] = readCaptions(document);
    expect(caption?.speaker).toBe("Grace Hopper");
    expect(caption?.speakerId).toBeUndefined();
  });
});

describe("readTranscriptPanel", () => {
  it("reads each transcript item with its author", () => {
    document.body.innerHTML = `
      <div data-tid="call-transcript-panel">
        <div data-tid="transcript-chat-message-abc/712"><span data-tid="transcript-item-author"></span>
          <span>Ada started transcription</span></div>
        <div data-tid="transcript-chat-message-abc/719">
          <span data-tid="transcript-item-author">Ada Lovelace</span><span>4:10 PM</span>
          <div data-tid="call-transcript-panel-message">And so we begin.</div>
        </div>
      </div>`;
    expect(readTranscriptPanel(document)).toEqual([
      { id: "abc/719", speaker: "Ada Lovelace", text: "And so we begin." },
    ]);
  });
});

describe("recap cards", () => {
  it("finds a recap card offering a transcript, with its meeting and times", () => {
    document.body.innerHTML = `
      <div class="card">
        <div><span>Weekly sync</span></div>
        <div>Monday, November 9, 2026 4:10 PM - 4:21 PM</div>
        <button>View recap</button>
        <div>Content</div><button><span>Transcript</span></button>
      </div>`;
    const [card] = findRecapCards(document);
    expect(card?.title).toBe("Weekly sync");
    expect(card?.hasTranscript).toBe(true);
    expect(card?.startMs).toBe(new Date(2026, 10, 9, 16, 10).getTime());
    expect(card?.endMs).toBe(new Date(2026, 10, 9, 16, 21).getTime());
  });

  it("parses recap time ranges, including midnight and noon", () => {
    expect(parseRecapTimeRange("Friday, January 2, 2026 12:05 AM - 12:30 PM")).toEqual({
      startMs: new Date(2026, 0, 2, 0, 5).getTime(),
      endMs: new Date(2026, 0, 2, 12, 30).getTime(),
    });
    expect(parseRecapTimeRange("no date here")).toBeNull();
  });
});
