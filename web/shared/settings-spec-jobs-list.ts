import type {
  SettingsWorkspaceSpecJobsResponse,
} from "../api/types";
import { requestJson } from "./api-client";

type SettingsSpecJobsFlight = {
  key: string;
  promise: Promise<SettingsWorkspaceSpecJobsResponse>;
};

const settingsSpecJobsListFlight: {
  current: SettingsSpecJobsFlight | null;
} = { current: null };

/**
 * Module-level single-flight for settings Spec jobs aggregate list.
 * Same query key reuses the in-flight request across callers/remounts.
 */
export function fetchSettingsWorkspaceSpecJobsList(params: {
  page: number;
  pageSize: number;
  retryableOnly: boolean;
}): Promise<SettingsWorkspaceSpecJobsResponse> {
  const flightKey = `${params.page}:${params.pageSize}:${params.retryableOnly}`;
  const existing = settingsSpecJobsListFlight.current;
  if (existing?.key === flightKey) {
    return existing.promise;
  }

  let promise: Promise<SettingsWorkspaceSpecJobsResponse> = Promise.resolve({
    jobs: [],
    errors: [],
    page: params.page,
    pageSize: params.pageSize,
    totalCount: 0,
    totalPages: 0,
  });
  promise = (async () => {
    try {
      const search = new URLSearchParams({
        page: String(params.page),
        pageSize: String(params.pageSize),
        retryableOnly: String(params.retryableOnly),
      });
      return await requestJson<SettingsWorkspaceSpecJobsResponse>(
        `/api/settings/spec/jobs?${search.toString()}`,
      );
    } finally {
      if (settingsSpecJobsListFlight.current?.promise === promise) {
        settingsSpecJobsListFlight.current = null;
      }
    }
  })();

  settingsSpecJobsListFlight.current = { key: flightKey, promise };
  return promise;
}
