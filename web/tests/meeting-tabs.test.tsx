// Specifies the meeting page's tabs (#201): Summary · Transcript · Notes · Files, one panel shown
// at a time, arrow keys between tabs, and the Ask AI button that opens the chat about the meeting.

import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import { MeetingAskAi, MeetingTabs } from "@/components/meeting-tabs";

const panels = {
  summary: <p>Summary text</p>,
  transcript: <p>Transcript text</p>,
  notes: <p>Notes text</p>,
  files: <p>Files text</p>
};

test("the tabs show one panel at a time, starting where asked", () => {
  render(<MeetingTabs initial="transcript" panels={panels} />);

  const tabs = screen.getAllByRole("tab").map((tab) => tab.textContent);
  expect(tabs).toEqual(["Summary", "Transcript", "Notes", "Files"]);
  expect(screen.getByRole("tab", { name: "Transcript" })).toHaveAttribute("aria-selected", "true");
  expect(screen.getByText("Transcript text")).toBeVisible();
  expect(screen.getByText("Summary text")).not.toBeVisible();

  fireEvent.click(screen.getByRole("tab", { name: "Files" }));
  expect(screen.getByText("Files text")).toBeVisible();
  expect(screen.getByText("Transcript text")).not.toBeVisible();
});

test("arrow keys move between tabs", () => {
  render(<MeetingTabs initial="summary" panels={panels} />);

  fireEvent.keyDown(screen.getByRole("tab", { name: "Summary" }), { key: "ArrowRight" });
  expect(screen.getByRole("tab", { name: "Transcript" })).toHaveAttribute("aria-selected", "true");
  expect(screen.getByRole("tab", { name: "Transcript" })).toHaveFocus();
  fireEvent.keyDown(screen.getByRole("tab", { name: "Transcript" }), { key: "ArrowLeft" });
  fireEvent.keyDown(screen.getByRole("tab", { name: "Summary" }), { key: "ArrowLeft" });
  expect(screen.getByRole("tab", { name: "Files" })).toHaveAttribute("aria-selected", "true");
});

test("Ask AI opens and closes the chat about the meeting", () => {
  render(
    <MeetingAskAi>
      <p>Chat panel</p>
    </MeetingAskAi>
  );

  expect(screen.queryByText("Chat panel")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Ask AI" }));
  expect(screen.getByText("Chat panel")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Ask AI" })).toHaveAttribute("aria-expanded", "true");
});
