import { describe, expect, it } from "vitest";

import type { ConfiguredModelSummary, ThinkingLevelSummary } from "../api/types";
import {
  defaultThinkingLevelForModel,
  isModelThinkingLevelSupported,
  normalizeThinkingLevelForModel,
  thinkingLevelOptionsForModel,
} from "./thinking-levels";

const thinkingLevels: ThinkingLevelSummary[] = [
  { label: "Minimal", value: "minimal" },
  { label: "Low", value: "low" },
  { label: "High", value: "high" },
];

function model(overrides: Partial<ConfiguredModelSummary>): ConfiguredModelSummary {
  return {
    activeProviderId: "openai",
    canEnable: true,
    contextWindow: 128000,
    displayName: "Test Model",
    enabled: true,
    id: "test-model",
    inputModalities: ["text"],
    metadataKey: null,
    outputModalities: ["text"],
    providerIds: ["openai"],
    supportsThinking: true,
    supportedThinkingLevels: ["low", "high"],
    thinkingLevel: "low",
    ...overrides,
  };
}

describe("thinking level helpers", () => {
  it("filters options to the model supported levels", () => {
    expect(thinkingLevelOptionsForModel(model({}), thinkingLevels)).toEqual([
      { label: "Low", value: "low" },
      { label: "High", value: "high" },
    ]);
  });

  it("drops unsupported and empty model levels", () => {
    expect(isModelThinkingLevelSupported(model({}), "minimal")).toBe(false);
    expect(normalizeThinkingLevelForModel(model({}), "minimal")).toBe("");
    expect(defaultThinkingLevelForModel(model({ thinkingLevel: "minimal" }))).toBe("");
    expect(
      thinkingLevelOptionsForModel(
        model({ supportedThinkingLevels: [], thinkingLevel: "low" }),
        thinkingLevels,
      ),
    ).toEqual([]);
  });
});
