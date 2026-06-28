import { describe, expect, it } from "vitest";

import { nextAutoRunnablePlan } from "./App";
import type { Plan } from "./api/types";

function plan(id: string, status: Plan["status"]): Plan {
  return {
    id,
    status,
    title: id,
    overview: "",
    sortOrder: 0,
    sourceChatId: null,
    activePhaseId: null,
    pauseRequestedAt: null,
    completedAt: null,
    completedByUserAt: null,
    errorMessage: null,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    phases: [],
  };
}

describe("nextAutoRunnablePlan", () => {
  it("starts the first draft, ready, or failed plan in list order", () => {
    expect(
      nextAutoRunnablePlan([
        plan("implemented", "implemented"),
        plan("failed", "failed"),
        plan("ready", "ready"),
      ]),
    ).toEqual({ planId: "failed", action: "start" });
  });

  it("resumes paused plans and ignores terminal plans", () => {
    expect(
      nextAutoRunnablePlan([
        plan("completed", "completed"),
        plan("cancelled", "cancelled"),
        plan("paused", "paused"),
      ]),
    ).toEqual({ planId: "paused", action: "resume" });
    expect(
      nextAutoRunnablePlan([
        plan("implemented", "implemented"),
        plan("completed", "completed"),
        plan("cancelled", "cancelled"),
      ]),
    ).toBeNull();
  });
});
