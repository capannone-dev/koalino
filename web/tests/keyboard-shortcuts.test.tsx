// Specifies the dashboard's keyboard shortcuts (#215): Ctrl/Cmd+K opens the command palette, which
// filters as you type and goes where Enter says; "?" lists the shortcuts; "/" puts the cursor in
// the page's search field; single keys are left alone while typing.

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";
import { KeyboardShortcuts } from "@/components/keyboard-shortcuts";

const push = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push }) }));

beforeEach(() => push.mockReset());

test("Ctrl+K opens the palette; typing filters and Enter goes there", () => {
  render(<KeyboardShortcuts />);
  fireEvent.keyDown(document, { key: "k", ctrlKey: true });

  const field = screen.getByRole("combobox", { name: "Type a command" });
  expect(field).toHaveFocus();
  expect(screen.getByRole("button", { name: "Go to Settings" })).toBeInTheDocument();
  fireEvent.change(field, { target: { value: "proj" } });
  expect(screen.queryByRole("button", { name: "Go to Settings" })).not.toBeInTheDocument();
  fireEvent.keyDown(field, { key: "Enter" });
  expect(push).toHaveBeenCalledWith("/projects");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

test("? lists the shortcuts and Escape closes them", () => {
  render(<KeyboardShortcuts />);
  fireEvent.keyDown(document.body, { key: "?" });
  expect(screen.getByRole("dialog", { name: "Keyboard shortcuts" })).toHaveTextContent("Play or pause the recording");
  fireEvent.keyDown(document.body, { key: "Escape" });
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

test("/ focuses the page's search field, and is text inside a field", () => {
  render(
    <>
      <KeyboardShortcuts />
      <input aria-label="Search meetings" data-shortcut-search="" />
      <textarea aria-label="Notes" />
    </>
  );
  fireEvent.keyDown(document.body, { key: "/" });
  expect(screen.getByLabelText("Search meetings")).toHaveFocus();

  fireEvent.keyDown(screen.getByLabelText("Notes"), { key: "?" });
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});
