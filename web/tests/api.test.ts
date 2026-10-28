// Verifies the typed API client calls the documented unknown-speaker cluster and meeting rename routes.

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { assignCluster, getUnknownClusters, MeetingRenameError, renameMeeting } from "@/lib/api";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

test("getUnknownClusters requests the meeting's cluster listing", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify([{ cluster_id: "c1" }]), { status: 200 }));

  const clusters = await getUnknownClusters("meeting 1");

  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/meetings/meeting%201/unknown-clusters", { cache: "no-store" });
  expect(clusters).toEqual([{ cluster_id: "c1" }]);
});

test("assignCluster posts the identity name and returns the assignment", async () => {
  const assignment = { cluster_id: "c1", identity_id: "i1", identity_name: "Grace", identity_created: true, variants_stored: 2, segments_updated: 1 };
  fetchMock.mockResolvedValue(new Response(JSON.stringify(assignment), { status: 200 }));

  await expect(assignCluster("c1", "Grace")).resolves.toEqual(assignment);
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/clusters/c1/assign", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ identity_name: "Grace" })
  });
});

test("assignCluster surfaces the server's error message", async () => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ error: "unknown-speaker cluster c1 is already resolved" }), { status: 409 }));

  await expect(assignCluster("c1", "Grace")).rejects.toThrow("Cluster assignment failed (409): unknown-speaker cluster c1 is already resolved");
});

test("renameMeeting patches the encoded meeting with the title and returns the summary", async () => {
  const summary = { meeting_id: "meeting 1", title: "Roadmap Review", created_at: "2026-10-27T09:05:00+00:00" };
  fetchMock.mockResolvedValue(new Response(JSON.stringify(summary), { status: 200 }));

  await expect(renameMeeting("meeting 1", "Roadmap Review")).resolves.toEqual(summary);
  expect(fetchMock).toHaveBeenCalledWith("http://127.0.0.1:8080/api/meetings/meeting%201", {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ title: "Roadmap Review" })
  });
});

test.each([
  [400, "invalid_title"],
  [404, "not_found"],
  [500, "server"],
  [503, "server"],
  [422, "unexpected"]
] as const)("renameMeeting maps HTTP %i to the %s failure", async (status, kind) => {
  fetchMock.mockResolvedValue(new Response(JSON.stringify({ error: "server detail" }), { status }));

  const failure = await renameMeeting("meeting-1", "Roadmap Review").catch((error: unknown) => error);

  expect(failure).toBeInstanceOf(MeetingRenameError);
  expect(failure).toMatchObject({ kind, status, serverMessage: "server detail" });
});

test("renameMeeting tolerates an error response without a JSON body", async () => {
  fetchMock.mockResolvedValue(new Response("Internal Server Error", { status: 500 }));

  await expect(renameMeeting("meeting-1", "Roadmap Review")).rejects.toMatchObject({ kind: "server", status: 500, serverMessage: null });
});

test("renameMeeting reports an unreachable daemon as a network failure", async () => {
  fetchMock.mockRejectedValue(new TypeError("Failed to fetch"));

  await expect(renameMeeting("meeting-1", "Roadmap Review")).rejects.toMatchObject({ kind: "network", status: null });
});
