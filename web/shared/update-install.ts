import type { UpdateStatusSummary } from "../api/types";
import { requestJson } from "./api-client";

type WaitForUpdateRestartOptions = {
  healthUrl?: string;
  pollMs?: number;
  reload?: () => void;
};

const UPDATE_RESTART_HEALTH_URL = "/api/health";
const UPDATE_RESTART_POLL_MS = 500;

export async function installUpdateAndWaitForRestart(
  options: WaitForUpdateRestartOptions = {},
) {
  const update = await requestJson<UpdateStatusSummary>("/api/update/install", {
    method: "POST",
  });
  void waitForUpdateRestartAndReload(options);
  return update;
}

export async function waitForUpdateRestartAndReload({
  healthUrl = UPDATE_RESTART_HEALTH_URL,
  pollMs = UPDATE_RESTART_POLL_MS,
  reload = () => window.location.reload(),
}: WaitForUpdateRestartOptions = {}) {
  while (await isUpdateHealthOk(healthUrl)) {
    await delay(pollMs);
  }

  while (!(await isUpdateHealthOk(healthUrl))) {
    await delay(pollMs);
  }

  reload();
}

async function isUpdateHealthOk(healthUrl: string) {
  try {
    const response = await fetch(healthUrl, {
      cache: "no-store",
      credentials: "same-origin",
    });
    return response.ok;
  } catch {
    return false;
  }
}

function delay(milliseconds: number) {
  return new Promise((resolve) => {
    window.setTimeout(resolve, milliseconds);
  });
}
