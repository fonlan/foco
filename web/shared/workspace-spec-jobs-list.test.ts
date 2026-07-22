import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { requestJsonMock } = vi.hoisted(() => ({
  requestJsonMock: vi.fn(),
}));

vi.mock("./api-client", () => ({
  requestJson: requestJsonMock,
}));

import { fetchWorkspaceSpecJobsList } from "./workspace-spec-jobs-list";

describe("fetchWorkspaceSpecJobsList", () => {
  beforeEach(() => {
    requestJsonMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("single-flights concurrent requests for the same workspace and limit", async () => {
    let resolveRequest: ((value: { jobs: [] }) => void) | undefined;
    requestJsonMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveRequest = resolve;
        }),
    );

    const first = fetchWorkspaceSpecJobsList("ws-concurrent", 24);
    const second = fetchWorkspaceSpecJobsList("ws-concurrent", 24);

    expect(requestJsonMock).toHaveBeenCalledTimes(1);
    expect(requestJsonMock).toHaveBeenCalledWith(
      "/api/workspaces/ws-concurrent/spec/jobs?limit=24",
    );

    resolveRequest?.({ jobs: [] });
    await expect(Promise.all([first, second])).resolves.toEqual([
      { jobs: [] },
      { jobs: [] },
    ]);
  });

  it("does not share flights across different workspaces or limits", async () => {
    requestJsonMock.mockResolvedValue({ jobs: [] });

    // Unique keys avoid cross-test storm-window reuse from earlier cases.
    await Promise.all([
      fetchWorkspaceSpecJobsList("ws-a-unique", 24),
      fetchWorkspaceSpecJobsList("ws-b-unique", 24),
      fetchWorkspaceSpecJobsList("ws-a-unique", 50),
    ]);

    expect(requestJsonMock).toHaveBeenCalledTimes(3);
  });

  it("reuses a just-settled response briefly to absorb request storms", async () => {
    requestJsonMock.mockResolvedValue({ jobs: [{ id: "job-1" }] });

    const first = await fetchWorkspaceSpecJobsList("ws-storm-unique", 24);
    const second = await fetchWorkspaceSpecJobsList("ws-storm-unique", 24);

    expect(first).toEqual({ jobs: [{ id: "job-1" }] });
    expect(second).toEqual({ jobs: [{ id: "job-1" }] });
    expect(requestJsonMock).toHaveBeenCalledTimes(1);

    await new Promise((resolve) => {
      window.setTimeout(resolve, 450);
    });
    await fetchWorkspaceSpecJobsList("ws-storm-unique", 24);
    expect(requestJsonMock).toHaveBeenCalledTimes(2);
  });
});
