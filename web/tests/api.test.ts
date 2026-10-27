// Verifies the typed API client calls the documented unknown-speaker cluster routes.

import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { assignCluster, getUnknownClusters } from "@/lib/api";

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
