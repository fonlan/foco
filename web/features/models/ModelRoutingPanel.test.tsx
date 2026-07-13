import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
} from "../../api/types";
import { MODEL_ROUTING_EXPANDED_STORAGE_KEY } from "../../app/constants";
import { I18nContext } from "../../shared/i18n";
import { ModelRoutingPanel } from "./ModelRoutingPanel";

const models: ConfiguredModelSummary[] = [
  {
    id: "gpt-4.1",
    displayName: "GPT-4.1",
    enabled: true,
    metadataKey: null,
    metadataSourceUrl: null,
    metadataRefreshedAt: null,
    contextWindow: 128000,
    maxOutputTokens: 8192,
    canEnable: true,
    missingLimits: [],
    providerIds: ["openai", "azure"],
    activeProviderId: "openai",
    inputModalities: ["text"],
    outputModalities: ["text"],
    thinkingLevel: null,
    systemPromptName: "Default",
    supportsThinking: false,
    supportedThinkingLevels: [],
    warnings: [],
  },
  {
    id: "claude",
    displayName: "Claude",
    enabled: false,
    metadataKey: null,
    metadataSourceUrl: null,
    metadataRefreshedAt: null,
    contextWindow: 200000,
    maxOutputTokens: 8192,
    canEnable: true,
    missingLimits: [],
    providerIds: ["anthropic"],
    activeProviderId: "anthropic",
    inputModalities: ["text"],
    outputModalities: ["text"],
    thinkingLevel: null,
    systemPromptName: "Default",
    supportsThinking: false,
    supportedThinkingLevels: [],
    warnings: [],
  },
];

const providers: ConfiguredProviderSummary[] = [
  {
    apiProxy: {
      enabled: false,
      proxyType: "http",
      supportedTypes: [
        { label: "HTTP", proxyType: "http" },
        { label: "SOCKS", proxyType: "socks" },
      ],
      url: "",
    },
    id: "openai",
    name: "OpenAI",
    kind: "openai-chat",
    kindLabel: "OpenAI Chat",
    enabled: true,
    baseUrl: null,
    hasApiKey: true,
    autoSyncModels: true,
    modelSyncFilterRegex: null,
    modelRedirects: [],
    requestOverrides: [],
    warnings: [],
  },
  {
    apiProxy: {
      enabled: false,
      proxyType: "http",
      supportedTypes: [
        { label: "HTTP", proxyType: "http" },
        { label: "SOCKS", proxyType: "socks" },
      ],
      url: "",
    },
    id: "azure",
    name: "Azure",
    kind: "openai-chat",
    kindLabel: "OpenAI Chat",
    enabled: true,
    baseUrl: null,
    hasApiKey: true,
    autoSyncModels: false,
    modelSyncFilterRegex: null,
    modelRedirects: [],
    requestOverrides: [],
    warnings: [],
  },
  {
    apiProxy: {
      enabled: false,
      proxyType: "http",
      supportedTypes: [
        { label: "HTTP", proxyType: "http" },
        { label: "SOCKS", proxyType: "socks" },
      ],
      url: "",
    },
    id: "anthropic",
    name: "Anthropic",
    kind: "openai-chat",
    kindLabel: "OpenAI Chat",
    enabled: false,
    baseUrl: null,
    hasApiKey: true,
    autoSyncModels: false,
    modelSyncFilterRegex: null,
    modelRedirects: [],
    requestOverrides: [],
    warnings: [],
  },
];

function renderPanel(
  onRouteChange = vi.fn(async () => ({ ok: true as const })),
) {
  return render(
    <I18nContext.Provider
      value={{
        language: "en",
        t: (key) => key,
      }}
    >
      <ModelRoutingPanel
        models={models}
        onRouteChange={onRouteChange}
        providers={providers}
      />
    </I18nContext.Provider>,
  );
}

describe("ModelRoutingPanel", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    cleanup();
  });

  it("keeps only the header when collapsed and expands the tree", () => {
    renderPanel();
    expect(screen.queryByRole("tree")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Model routing" }));
    expect(screen.getByRole("tree")).toBeTruthy();
    expect(window.localStorage.getItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY)).toBe(
      "1",
    );
  });

  it("routes a model through a provider and reports errors", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    const onRouteChange = vi.fn(async () => ({
      ok: false as const,
      error: "route failed",
    }));
    renderPanel(onRouteChange);

    fireEvent.click(screen.getByRole("button", { name: /GPT-4.1/ }));
    fireEvent.click(screen.getByRole("radio", { name: /Azure/ }));

    await waitFor(() => {
      expect(onRouteChange).toHaveBeenCalledWith("gpt-4.1", "azure");
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("route failed");
  });

  it("disables unavailable providers", () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    renderPanel();
    fireEvent.click(screen.getByRole("button", { name: /Claude/ }));
    const anthropic = screen.getByRole("radio", { name: /Anthropic/ });
    expect(anthropic).toBeDisabled();
  });
});
