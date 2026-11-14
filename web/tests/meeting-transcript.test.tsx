// Verifies the meeting page's transcript shows each segment's speaker, and marks guessed
// speakers, overlapping speech and unclear words (#90, #91).

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { MeetingTranscript } from "@/components/meeting-transcript";
import * as edits from "@/lib/edits-api";

vi.mock("@/lib/edits-api", async () => ({
  ...(await vi.importActual<typeof edits>("@/lib/edits-api")),
  editSegment: vi.fn()
}));

const doubtful = {
  start_time_ms: 61_000,
  end_time_ms: 65_000,
  speaker: "Speaker 1",
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
  expect(screen.getByText("Speaker 1 (?) · 1:01")).toHaveAttribute("title", "The speaker is a guess");
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

describe("editing (#96)", () => {
  beforeEach(() => vi.mocked(edits.editSegment).mockReset());

  test("a segment is corrected in place and keeps its speaker", async () => {
    vi.mocked(edits.editSegment).mockResolvedValue({ start_time_ms: 61_000, end_time_ms: 65_000, text: "we ship on Monday" });
    render(<MeetingTranscript meetingId="m-1" transcript={null} segments={[doubtful]} />);

    fireEvent.click(screen.getByRole("button", { name: "Edit the text at 1:01" }));
    const field = screen.getByLabelText("Text at 1:01");
    expect(field).toHaveValue("we ship on Friday");
    fireEvent.change(field, { target: { value: "we ship on Monday" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("status")).toHaveTextContent(/won't be re-transcribed/);
    expect(screen.getByText("we ship on Monday", { selector: "p" })).toBeInTheDocument();
    expect(edits.editSegment).toHaveBeenCalledWith("m-1", 61_000, 65_000, "we ship on Monday");
    expect(screen.getByText("Speaker 1 (?) · 1:01")).toBeInTheDocument();
  });

  test("an emptied segment disappears, and a failure keeps the text", async () => {
    vi.mocked(edits.editSegment).mockRejectedValueOnce(new edits.EditApiError("segment_not_found", 404));
    render(<MeetingTranscript meetingId="m-1" transcript={null} segments={[doubtful]} />);

    fireEvent.click(screen.getByRole("button", { name: "Edit the text at 1:01" }));
    fireEvent.change(screen.getByLabelText("Text at 1:01"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByText(/changed\. Reload the page/)).toBeInTheDocument();
    expect(screen.getByLabelText("Text at 1:01")).toBeInTheDocument();

    vi.mocked(edits.editSegment).mockResolvedValueOnce(null);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByText("Speaker 1 (?) · 1:01")).not.toBeInTheDocument());
  });

  test("without a meeting ID the transcript is read-only", () => {
    render(<MeetingTranscript transcript={null} segments={[doubtful]} />);
    expect(screen.queryByRole("button", { name: /Edit the text/ })).not.toBeInTheDocument();
  });
});

test("a cited moment (#at-<ms>) scrolls to the passage it falls in (#110)", () => {
  const scrolled: string[] = [];
  const original = Element.prototype.scrollIntoView;
  Element.prototype.scrollIntoView = function (this: Element) {
    scrolled.push(this.id);
  };
  window.location.hash = "#at-63000";
  try {
    render(
      <MeetingTranscript
        transcript={null}
        segments={[
          { ...doubtful, start_time_ms: 0, end_time_ms: 61_000, text: "Hello." },
          doubtful,
          { ...doubtful, start_time_ms: 65_000, end_time_ms: 70_000, text: "Bye." }
        ]}
      />
    );
    expect(scrolled).toEqual(["t-61000"]);
  } finally {
    Element.prototype.scrollIntoView = original;
    window.location.hash = "";
  }
});
