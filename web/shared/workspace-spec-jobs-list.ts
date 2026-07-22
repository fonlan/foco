import type { WorkspaceSpecJobsResponse } from "../api/types";
import { requestJson } from "./api-client";

const REQUEST_STORM_DEDUPE_MS = 400;

type SingleFlightEntry<T> = {
  promise: Promise<T>;
  settled: boolean;
  startedAtMs: number;
};

const workspaceSpecJobsListFlights = new Map<
  string,
  SingleFlightEntry<WorkspaceSpecJobsResponse>
>();

function nowMs() {
  return typeof performance === "undefined" ? Date.now() : performance.now();
}

function shouldReuseRequest<T>(
  entry: SingleFlightEntry<T> | undefined,
  currentMs: number,
) {
  if (!entry) {
    return false;
  }
  return (
    !entry.settled || currentMs - entry.startedAtMs < REQUEST_STORM_DEDUPE_MS
  );
}

/**
 * Module-level single-flight for workspace Spec jobs list.
 * Shared by App poll observers (and any other consumer) so concurrent UI
 * paths reuse one in-flight `/api/workspaces/{id}/spec/jobs` request.
 */
export function fetchWorkspaceSpecJobsList(
  workspaceId: string,
  limit = 24,
): Promise<WorkspaceSpecJobsResponse> {
  const flightKey = `${workspaceId}:${limit}`;
  const currentMs = nowMs();
  const existing = workspaceSpecJobsListFlights.get(flightKey);
  if (shouldReuseRequest(existing, currentMs)) {
    return existing!.promise;
  }

  let promise: Promise<WorkspaceSpecJobsResponse> = Promise.resolve({
    jobs: [],
  });
  promise = (async () => {
    try {
      return await requestJson<WorkspaceSpecJobsResponse>(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/spec/jobs?limit=${limit}`,
      );
    } finally {
      const current = workspaceSpecJobsListFlights.get(flightKey);
      if (current?.promise === promise) {
        current.settled = true;
        window.setTimeout(() => {
          if (workspaceSpecJobsListFlights.get(flightKey)?.promise === promise) {
            workspaceSpecJobsListFlights.delete(flightKey);
          }
        }, REQUEST_STORM_DEDUPE_MS);
      }
    }
  })();

  workspaceSpecJobsListFlights.set(flightKey, {
    promise,
    settled: false,
    startedAtMs: currentMs,
  });
  return promise;
}
