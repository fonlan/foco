import { beforeEach, describe, expect, it, vi } from "vitest";

const { requestJsonMock } = vi.hoisted(() => ({
  requestJsonMock: vi.fn(),
}));

vi.mock("./api-client", () => ({
  requestJson: requestJsonMock,
}));

import { fetchSettingsWorkspaceSpecJobsList } from "./settings-spec-jobs-list";

describe("fetchSettingsWorkspaceSpecJobsList", () => {
  beforeEach(() => {
    requestJsonMock.mockReset();
  });

  it("single-flights concurrent requests for the same query key", async () => {
    let resolveRequest:
      | ((value: {
          jobs: [];
          errors: [];
          page: number;
          pageSize: number;
          totalCount: number;
          totalPages: number;
        }) => void)
      | undefined;
    requestJsonMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveRequest = resolve;
        }),
    );

    const params = { page: 1, pageSize: 20, retryableOnly: false };
    const first = fetchSettingsWorkspaceSpecJobsList(params);
    const second = fetchSettingsWorkspaceSpecJobsList(params);

    expect(requestJsonMock).toHaveBeenCalledTimes(1);
    expect(requestJsonMock).toHaveBeenCalledWith(
      "/api/settings/spec/jobs?page=1&pageSize=20&retryableOnly=false",
    );

    resolveRequest?.({
      jobs: [],
      errors: [],
      page: 1,
      pageSize: 20,
      totalCount: 0,
      totalPages: 0,
    });
    await expect(Promise.all([first, second])).resolves.toEqual([
      {
        jobs: [],
        errors: [],
        page: 1,
        pageSize: 20,
        totalCount: 0,
        totalPages: 0,
      },
      {
        jobs: [],
        errors: [],
        page: 1,
        pageSize: 20,
        totalCount: 0,
        totalPages: 0,
      },
    ]);
  });

  it("issues a new request when the query key changes", async () => {
    requestJsonMock.mockResolvedValue({
      jobs: [],
      errors: [],
      page: 1,
      pageSize: 20,
      totalCount: 0,
      totalPages: 0,
    });

    await fetchSettingsWorkspaceSpecJobsList({
      page: 1,
      pageSize: 20,
      retryableOnly: false,
    });
    await fetchSettingsWorkspaceSpecJobsList({
      page: 2,
      pageSize: 20,
      retryableOnly: false,
    });
    await fetchSettingsWorkspaceSpecJobsList({
      page: 1,
      pageSize: 20,
      retryableOnly: true,
    });

    expect(requestJsonMock).toHaveBeenCalledTimes(3);
  });

  it("allows a fresh request after the previous flight settles", async () => {
    requestJsonMock.mockResolvedValue({
      jobs: [],
      errors: [],
      page: 1,
      pageSize: 20,
      totalCount: 0,
      totalPages: 0,
    });

    await fetchSettingsWorkspaceSpecJobsList({
      page: 1,
      pageSize: 20,
      retryableOnly: false,
    });
    await fetchSettingsWorkspaceSpecJobsList({
      page: 1,
      pageSize: 20,
      retryableOnly: false,
    });

    expect(requestJsonMock).toHaveBeenCalledTimes(2);
  });
});
