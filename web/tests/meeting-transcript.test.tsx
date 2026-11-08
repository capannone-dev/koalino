// Verifies the meeting page's transcript shows each segment's speaker, and marks guessed
// speakers, overlapping speech and unclear words (#90, #91).

import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { MeetingTranscript } from "@/components/meeting-transcript";

const doubtful = {
  start_time_ms: 61_000,
  end_time_ms: 65_000,
  speaker: "Unknown speaker",
  text: "we ship on Friday",
  overlapping_speech: true,
  uncertain_speaker: true,
  uncertain_words: [1]
};

test("segments show their speaker, start and text", () => {
  render(
    <MeetingTranscript
      transcript="Hello team."
      segments={[{ ...doubtful, overlapping_speech: false, uncertain_speaker: false, uncertain_words: [], speaker: "Alice", text: "Hello team.", start_time_ms: 0 }]}
    />
  );
  expect(screen.getByText("Alice · 0:00")).toBeInTheDocument();
  expect(screen.getByText("Hello team.")).toBeInTheDocument();
  expect(screen.queryByText(/overlapping speech/i)).not.toBeInTheDocument();
});

test("a guessed speaker, overlap and unclear words are marked", () => {
  render(<MeetingTranscript transcript={null} segments={[doubtful]} />);
  expect(screen.getByText("Unknown speaker (?) · 1:01")).toHaveAttribute("title", "The speaker is a guess");
  expect(screen.getByText("Overlapping speech")).toBeInTheDocument();
  const unclear = screen.getByText("ship");
  expect(unclear.tagName).toBe("EM");
  expect(unclear).toHaveAttribute("title", "Unclear word");
  expect(screen.getByText("Friday").tagName).not.toBe("EM");
});

test("without segments the flat transcript or a message is shown", () => {
  const { rerender } = render(<MeetingTranscript transcript="Imported text." segments={[]} />);
  expect(screen.getByText("Imported text.")).toBeInTheDocument();
  rerender(<MeetingTranscript transcript={null} />);
  expect(screen.getByText("No transcript is available.")).toBeInTheDocument();
});
