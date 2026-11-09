// Verifies search on the web (#100): the request per scope, results that say where each match is
// and link there with the matched words marked, and failures explained.

import { fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { SearchPanel } from "@/components/search-panel";
import { hitHref, searchNotes, SearchApiError, type SearchHit } from "@/lib/search-api";

const transcriptHit: SearchHit = {
  place: "transcript",
  start_time_ms: 61000,
  end_time_ms: 64000,
  speaker: "Anna",
  meeting: { meeting_id: "m 1", title: "Budget review", created_at: "2026-11-09T10:00:00Z" },
  project: null,
  resource: null,
  snippet: [
    { text: "The ", match: false },
    { text: "budget", match: true },
    { text: " is approved.", match: false }
  ]
};

const fileHit: SearchHit = {
  place: "resource_text",
  meeting: null,
  project: { project_id: "p-1", name: "Launch" },
  resource: { attachment_id: "a-1", file_name: "plan.md", mime_type: "text/markdown", kind: "markdown" },
  snippet: [{ text: "budget", match: true }]
};

afterEach(() => vi.unstubAllGlobals());

function answer(body: unknown, status = 200) {
  return vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { status }));
}

test("the request carries the query and the scope", async () => {
  const fetchMock = answer({ ok: true, hits: [], truncated: false });
  vi.stubGlobal("fetch", fetchMock);

  await searchNotes("budget q3", "project", "p 1");

  const url = new URL(String(fetchMock.mock.calls[0][0]));
  expect(url.pathname).toBe("/api/search");
  expect(url.searchParams.get("q")).toBe("budget q3");
  expect(url.searchParams.get("scope")).toBe("project");
  expect(url.searchParams.get("project")).toBe("p 1");
});

test("a refusal becomes a SearchApiError", async () => {
  vi.stubGlobal("fetch", answer({ ok: false, error_code: "too_short" }, 400));
  await expect(searchNotes("x", "meetings")).rejects.toEqual(new SearchApiError("too_short", 400));
});

test("hits link to the moment or the file", () => {
  expect(hitHref(transcriptHit)).toBe("/meetings/m%201#t-61000");
  expect(hitHref({ ...transcriptHit, place: "notes" })).toBe("/meetings/m%201#notes");
  expect(hitHref({ ...transcriptHit, place: "summary" })).toBe("/meetings/m%201#summary");
  expect(hitHref(fileHit)).toBe("/projects/p-1#file-a-1");
  expect(hitHref({ ...fileHit, place: "project_name", resource: null })).toBe("/projects/p-1");
});

test("results say where each match is and mark the words", async () => {
  vi.stubGlobal("fetch", answer({ ok: true, hits: [transcriptHit, fileHit], truncated: false }));
  render(<SearchPanel scope="projects" label="Search projects" />);

  fireEvent.change(screen.getByLabelText("Search projects"), { target: { value: "budget" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));

  expect(await screen.findByText("2 results.")).toBeInTheDocument();
  const items = screen.getAllByRole("listitem");
  const first = within(items[0]);
  expect(first.getByRole("link", { name: /Budget review/ })).toHaveAttribute("href", "/meetings/m%201#t-61000");
  expect(first.getByText("1:01 · Anna")).toBeInTheDocument();
  expect(first.getByText("budget", { selector: "mark" })).toBeInTheDocument();
  const second = within(items[1]);
  expect(second.getByRole("link", { name: /Launch/ })).toHaveAttribute("href", "/projects/p-1#file-a-1");
  expect(second.getByText("In plan.md")).toBeInTheDocument();
});

test("no results and failures are said plainly", async () => {
  vi.stubGlobal("fetch", answer({ ok: true, hits: [], truncated: false }));
  render(<SearchPanel scope="meetings" label="Search meetings" />);
  fireEvent.change(screen.getByLabelText("Search meetings"), { target: { value: "courier" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  expect(await screen.findByText("Nothing found.")).toBeInTheDocument();

  vi.stubGlobal("fetch", answer({ ok: false, error_code: "too_short" }, 400));
  fireEvent.change(screen.getByLabelText("Search meetings"), { target: { value: "c" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  expect(await screen.findByText("Type at least two letters.")).toBeInTheDocument();
});
