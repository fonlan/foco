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
  { label: "Medium", value: "medium" },
  { label: "High", value: "high" },
  { label: "XHigh", value: "xhigh" },
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
  it("does not show Minimal when the model only supports low through xhigh", () => {
    expect(
      thinkingLevelOptionsForModel(
        model({
          id: "gpt-5.5",
          supportedThinkingLevels: ["low", "medium", "high", "xhigh"],
        }),
        thinkingLevels,
      ),
    ).toEqual([
      { label: "Low", value: "low" },
      { label: "Medium", value: "medium" },
      { label: "High", value: "high" },
      { label: "XHigh", value: "xhigh" },
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
