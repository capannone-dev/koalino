// Verifies the route error boundary explains an unreachable daemon in plain words, keeps the
// technical hint folded for the administrator (#203), and offers a retry.

import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import RouteError from "@/app/error";

test("explains that the daemon could not be reached and hides the raw error", () => {
  render(<RouteError error={new Error("Axum API request failed: 500 /internal/path")} reset={() => {}} />);

  expect(screen.getByRole("heading", { name: "Can't load this page" })).toBeInTheDocument();
  expect(screen.getByText(/note-taking server/)).toBeInTheDocument();
  expect(screen.queryByText(/internal\/path/)).not.toBeInTheDocument();
});

test("Try again re-renders the route", () => {
  const reset = vi.fn();
  render(<RouteError error={new Error("fetch failed")} reset={reset} />);

  fireEvent.click(screen.getByRole("button", { name: "Try again" }));

  expect(reset).toHaveBeenCalledTimes(1);
});

test("the error speaks plainly, keeps the technical hint folded, and retries", () => {
  const reset = vi.fn();
  render(<RouteError error={new Error("boom")} reset={reset} />);
  expect(screen.getByRole("alert")).toHaveTextContent("Koalino on this server isn't responding. Wait a moment, then try again.");
  const details = screen.getByText("Details for the administrator").closest("details");
  expect(details).not.toHaveAttribute("open");
  expect(details).toHaveTextContent("NEXT_PUBLIC_API_BASE_URL");
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  expect(reset).toHaveBeenCalled();
});
