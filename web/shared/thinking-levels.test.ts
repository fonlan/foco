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
  { label: "Maximum", value: "max" },
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

  it("shows max only when model metadata declares it", () => {
    const gpt56 = model({
      id: "gpt-5.6",
      supportedThinkingLevels: ["low", "high", "xhigh", "max"],
      thinkingLevel: "max",
    });

    expect(thinkingLevelOptionsForModel(gpt56, thinkingLevels)).toEqual([
      { label: "Low", value: "low" },
      { label: "High", value: "high" },
      { label: "XHigh", value: "xhigh" },
      { label: "Maximum", value: "max" },
    ]);
    expect(defaultThinkingLevelForModel(gpt56)).toBe("max");
    expect(normalizeThinkingLevelForModel(gpt56, "max")).toBe("max");

    const withoutMax = model({
      supportedThinkingLevels: ["low", "high", "xhigh"],
      thinkingLevel: "max",
    });
    expect(thinkingLevelOptionsForModel(withoutMax, thinkingLevels)).not.toContainEqual({
      label: "Maximum",
      value: "max",
    });
    expect(defaultThinkingLevelForModel(withoutMax)).toBe("");
    expect(normalizeThinkingLevelForModel(withoutMax, "max")).toBe("");
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
