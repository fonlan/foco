import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
} from "../../api/types";
import {
  DEFAULT_MODEL_ROUTING_HEIGHT_RATIO,
  MODEL_ROUTING_EXPANDED_STORAGE_KEY,
  MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY,
  MODEL_ROUTING_PANEL_MIN_HEIGHT_PX,
  MODEL_ROUTING_RESIZE_STEP_PX,
  WORKSPACE_NAV_MIN_HEIGHT_PX,
} from "../../app/constants";
import { I18nContext } from "../../shared/i18n";
import { ModelRoutingPanel } from "./ModelRoutingPanel";
import {
  clampModelRoutingPanelHeight,
  modelRoutingPanelHeightBounds,
  normalizeModelRoutingHeightRatio,
  panelHeightFromRatio,
  ratioFromPanelHeight,
  readModelRoutingHeightRatio,
} from "./model-routing-preferences";

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

function mockRect(height: number, top = 0): DOMRect {
  return {
    bottom: top + height,
    height,
    left: 0,
    right: 320,
    toJSON: () => ({}),
    top,
    width: 320,
    x: 0,
    y: top,
  } as DOMRect;
}

function mockFlexibleStackLayout(
  container: HTMLElement,
  {
    availableHeight = 500,
    panelHeight,
  }: {
    availableHeight?: number;
    panelHeight?: number;
  } = {},
) {
  const panel = container.querySelector(
    ".model-routing-panel",
  ) as HTMLElement | null;
  const nav = container.querySelector(".workspace-nav") as HTMLElement | null;
  if (!panel || !nav) {
    throw new Error("Expected workspace-nav sibling and model-routing-panel");
  }

  const resolvedPanelHeight =
    panelHeight ??
    panelHeightFromRatio(DEFAULT_MODEL_ROUTING_HEIGHT_RATIO, availableHeight);
  const navHeight = availableHeight - resolvedPanelHeight;

  vi.spyOn(nav, "getBoundingClientRect").mockReturnValue(mockRect(navHeight, 0));
  vi.spyOn(panel, "getBoundingClientRect").mockReturnValue(
    mockRect(resolvedPanelHeight, navHeight),
  );

  return { nav, panel, panelHeight: resolvedPanelHeight, availableHeight };
}

function renderPanel(
  onRouteChange = vi.fn(async () => ({ ok: true as const })),
  {
    onFastModeChange = vi.fn(async () => ({ ok: true as const })),
    panelModels = models,
    panelProviders = providers,
  }: {
    onFastModeChange?: (
      modelId: string,
      fastModeEnabled: boolean,
    ) => Promise<{ ok: true } | { ok: false; error: string }>;
    panelModels?: ConfiguredModelSummary[];
    panelProviders?: ConfiguredProviderSummary[];
  } = {},
) {
  return render(
    <I18nContext.Provider
      value={{
        language: "en",
        t: (key, variables) =>
          key.startsWith("Enable Fast mode") || key.startsWith("Disable Fast mode")
            ? key.replace("{name}", String(variables?.name ?? "{name}"))
            : key,
      }}
    >
      <div className="flex h-full min-h-0 flex-col" style={{ height: 600 }}>
        <nav
          aria-label="Workspace list"
          className="workspace-nav panel-scroll min-h-0 flex-1 overflow-y-auto"
        />
        <ModelRoutingPanel
          models={panelModels}
          onFastModeChange={onFastModeChange}
          onRouteChange={onRouteChange}
          providers={panelProviders}
        />
      </div>
    </I18nContext.Provider>,
  );
}

describe("model-routing-preferences height ratio", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  it("falls back to the default ratio for invalid storage values", () => {
    expect(normalizeModelRoutingHeightRatio(Number.NaN)).toBe(
      DEFAULT_MODEL_ROUTING_HEIGHT_RATIO,
    );
    expect(normalizeModelRoutingHeightRatio(-0.2)).toBe(
      DEFAULT_MODEL_ROUTING_HEIGHT_RATIO,
    );
    expect(normalizeModelRoutingHeightRatio(0)).toBe(
      DEFAULT_MODEL_ROUTING_HEIGHT_RATIO,
    );
    expect(normalizeModelRoutingHeightRatio(1.1)).toBe(
      DEFAULT_MODEL_ROUTING_HEIGHT_RATIO,
    );
    expect(normalizeModelRoutingHeightRatio("nope")).toBe(
      DEFAULT_MODEL_ROUTING_HEIGHT_RATIO,
    );
    expect(normalizeModelRoutingHeightRatio(0.55)).toBe(0.55);
    // Full-stack share is a legitimate extreme, not invalid input.
    expect(normalizeModelRoutingHeightRatio(1)).toBe(1);

    window.localStorage.setItem(MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY, "abc");
    expect(readModelRoutingHeightRatio()).toBe(DEFAULT_MODEL_ROUTING_HEIGHT_RATIO);
  });

  it("clamps panel height against both flexible-stack mins", () => {
    expect(clampModelRoutingPanelHeight(50, 500)).toBe(
      MODEL_ROUTING_PANEL_MIN_HEIGHT_PX,
    );
    expect(clampModelRoutingPanelHeight(480, 500)).toBe(
      500 - WORKSPACE_NAV_MIN_HEIGHT_PX,
    );
    expect(clampModelRoutingPanelHeight(200, 500)).toBe(200);
  });

  it("does not force panel min when the flexible stack is shorter than both mins", () => {
    // available=200 < 120+120: reserve nav min, panel max becomes 80 (no overflow).
    expect(clampModelRoutingPanelHeight(120, 200)).toBe(80);
    expect(clampModelRoutingPanelHeight(50, 200)).toBe(80);
    expect(modelRoutingPanelHeightBounds(200)).toEqual({ min: 80, max: 80 });

    // available=100: nav takes all remaining, panel collapses to 0.
    expect(clampModelRoutingPanelHeight(50, 100)).toBe(0);
    expect(modelRoutingPanelHeightBounds(100)).toEqual({ min: 0, max: 0 });
  });

  it("keeps extreme panel shares as durable ratios instead of snapping to default", () => {
    expect(ratioFromPanelHeight(200, 200)).toBe(1);
    expect(ratioFromPanelHeight(80, 200)).toBeCloseTo(0.4, 5);
    expect(ratioFromPanelHeight(120, 500)).toBeCloseTo(0.24, 5);
  });
});

describe("ModelRoutingPanel", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    cleanup();
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  });

  it("keeps only the header when collapsed and expands the tree", () => {
    renderPanel();
    expect(screen.queryByRole("tree")).toBeNull();
    expect(
      screen.queryByRole("separator", { name: "Resize model routing panel" }),
    ).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Model routing" }));
    expect(screen.getByRole("tree")).toBeTruthy();
    expect(
      screen.getByRole("separator", { name: "Resize model routing panel" }),
    ).toBeTruthy();
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
    fireEvent.click(screen.getByRole("button", { name: "Azure" }));

    await waitFor(() => {
      expect(onRouteChange).toHaveBeenCalledWith("gpt-4.1", "azure");
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("route failed");
  });

  it("shows an accessible Fast icon button only for eligible models", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    const onFastModeChange = vi.fn(async () => ({ ok: true as const }));
    renderPanel(undefined, {
      onFastModeChange,
      panelModels: [
        { ...models[0]!, fastModeEnabled: false, supportsFast: true },
        { ...models[1]!, fastModeEnabled: true, supportsFast: false },
      ],
    });

    const fastToggle = screen.getByRole("button", {
      name: "Enable Fast mode for GPT-4.1",
    });
    expect(fastToggle).toHaveAttribute("aria-pressed", "false");
    expect(fastToggle).toHaveAttribute(
      "title",
      "Enable Fast mode for GPT-4.1",
    );
    expect(
      screen.queryByRole("button", { name: /Fast mode.*Claude/ }),
    ).toBeNull();

    await act(async () => {
      fireEvent.click(fastToggle);
    });
    await waitFor(() => {
      expect(onFastModeChange).toHaveBeenCalledWith("gpt-4.1", true);
    });
    expect(screen.queryByRole("group")).toBeNull();
    expect(fastToggle.parentElement?.closest("button")).toBeNull();
  });

  it("reflects an enabled Fast preference and disables its icon while the update is pending", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    let resolveUpdate: (result: { ok: true } | { ok: false; error: string }) => void =
      () => undefined;
    const onFastModeChange = vi.fn(
      () =>
        new Promise<{ ok: true } | { ok: false; error: string }>((resolve) => {
          resolveUpdate = resolve;
        }),
    );
    renderPanel(undefined, {
      onFastModeChange,
      panelModels: [{ ...models[0]!, fastModeEnabled: true, supportsFast: true }],
    });

    const fastToggle = screen.getByRole("button", {
      name: "Disable Fast mode for GPT-4.1",
    });
    expect(fastToggle).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(fastToggle);
    await waitFor(() => {
      expect(onFastModeChange).toHaveBeenCalledWith("gpt-4.1", false);
    });
    expect(fastToggle).toBeDisabled();

    await act(async () => {
      resolveUpdate({ ok: true });
    });
    await waitFor(() => expect(fastToggle).not.toBeDisabled());
  });

  it("disables Fast for unavailable eligible models", () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    const onFastModeChange = vi.fn(async () => ({ ok: true as const }));
    renderPanel(undefined, {
      onFastModeChange,
      panelModels: [
        { ...models[0]!, enabled: false, fastModeEnabled: false, supportsFast: true },
      ],
    });

    const fastToggle = screen.getByRole("button", {
      name: "Enable Fast mode for GPT-4.1",
    });
    expect(fastToggle).toBeDisabled();
    fireEvent.click(fastToggle);
    expect(onFastModeChange).not.toHaveBeenCalled();
  });

  it("rolls back a failed Fast preference update and exposes the panel error", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    const onFastModeChange = vi.fn(async () => ({
      ok: false as const,
      error: "Fast preference failed",
    }));
    renderPanel(undefined, {
      onFastModeChange,
      panelModels: [{ ...models[0]!, fastModeEnabled: false, supportsFast: true }],
    });

    const fastToggle = screen.getByRole("button", {
      name: "Enable Fast mode for GPT-4.1",
    });
    fireEvent.click(fastToggle);
    expect(fastToggle).toBeDisabled();

    await waitFor(() => {
      expect(
        screen.getByRole("button", {
          name: "Enable Fast mode for GPT-4.1",
        }),
      ).toHaveAttribute("aria-pressed", "false");
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Fast preference failed",
    );
  });

  it("optimistically selects the provider before the request settles", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    let resolveRoute: (value: { ok: true } | { ok: false; error: string }) => void =
      () => undefined;
    const onRouteChange = vi.fn(
      () =>
        new Promise<{ ok: true } | { ok: false; error: string }>((resolve) => {
          resolveRoute = resolve;
        }),
    );
    renderPanel(onRouteChange);

    fireEvent.click(screen.getByRole("button", { name: /GPT-4.1/ }));
    fireEvent.click(screen.getByRole("button", { name: "Azure" }));

    await waitFor(() => {
      expect(onRouteChange).toHaveBeenCalledWith("gpt-4.1", "azure");
    });

    const azure = screen.getByRole("button", { name: "Azure" });
    expect(azure).toHaveAttribute("aria-current", "true");
    expect(azure).toHaveClass("model-routing-provider-active");
    expect(
      screen.getByRole("button", { name: /GPT-4\.1.*Azure/ }),
    ).toBeInTheDocument();

    resolveRoute({ ok: true });
    // Without a parent models update, optimistic overlay clears after settle and
    // props (openai) reappear — App.tsx owns the durable activeProviderId.
    await waitFor(() => {
      expect(screen.getByRole("button", { name: "OpenAI" })).toHaveAttribute(
        "aria-current",
        "true",
      );
    });
  });

  it("rolls back optimistic selection when routing fails", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    let resolveRoute: (value: { ok: true } | { ok: false; error: string }) => void =
      () => undefined;
    const onRouteChange = vi.fn(
      () =>
        new Promise<{ ok: true } | { ok: false; error: string }>((resolve) => {
          resolveRoute = resolve;
        }),
    );
    renderPanel(onRouteChange);

    fireEvent.click(screen.getByRole("button", { name: /GPT-4.1/ }));
    fireEvent.click(screen.getByRole("button", { name: "Azure" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Azure" })).toHaveAttribute(
        "aria-current",
        "true",
      );
    });

    resolveRoute({ ok: false, error: "route failed" });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "OpenAI" })).toHaveAttribute(
        "aria-current",
        "true",
      );
    });
    expect(screen.getByRole("button", { name: "Azure" })).not.toHaveAttribute(
      "aria-current",
    );
    expect(await screen.findByRole("alert")).toHaveTextContent("route failed");
  });

  it("ignores concurrent clicks on the same model while routing", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    let resolveRoute: (value: { ok: true } | { ok: false; error: string }) => void =
      () => undefined;
    const onRouteChange = vi.fn(
      () =>
        new Promise<{ ok: true } | { ok: false; error: string }>((resolve) => {
          resolveRoute = resolve;
        }),
    );
    renderPanel(onRouteChange);

    fireEvent.click(screen.getByRole("button", { name: /GPT-4.1/ }));
    fireEvent.click(screen.getByRole("button", { name: "Azure" }));
    // Second click is ignored while the first request is in flight (busy lock).
    fireEvent.click(screen.getByRole("button", { name: "OpenAI" }));

    await waitFor(() => {
      expect(onRouteChange).toHaveBeenCalledTimes(1);
    });
    expect(onRouteChange).toHaveBeenCalledWith("gpt-4.1", "azure");
    expect(screen.getByRole("button", { name: "Azure" })).toHaveAttribute(
      "aria-current",
      "true",
    );

    resolveRoute({ ok: true });
    await waitFor(() => {
      expect(onRouteChange).toHaveBeenCalledTimes(1);
    });
  });

  it("hides disabled providers while keeping enabled routes selectable", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    const onRouteChange = vi.fn(async () => ({ ok: true as const }));
    renderPanel(onRouteChange, {
      panelModels: [
        {
          ...models[0],
          activeProviderId: "anthropic",
          providerIds: ["anthropic", "azure"],
        },
      ],
    });

    expect(
      screen.getByRole("button", { name: /GPT-4\.1.*Anthropic/ }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /GPT-4.1/ }));

    expect(screen.queryByRole("button", { name: "Anthropic" })).toBeNull();
    const azure = screen.getByRole("button", { name: "Azure" });
    expect(azure).not.toBeDisabled();

    fireEvent.click(azure);
    await waitFor(() => {
      expect(onRouteChange).toHaveBeenCalledWith("gpt-4.1", "azure");
    });
  });

  it("shows the linked-provider empty state after filtering disabled providers", () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    renderPanel(undefined, {
      panelModels: [
        {
          ...models[0],
          activeProviderId: "anthropic",
          providerIds: ["anthropic"],
        },
      ],
    });

    expect(
      screen.getByRole("button", { name: /GPT-4\.1.*Anthropic/ }),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /GPT-4.1/ }));

    expect(screen.getByText("No linked providers")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Anthropic" })).toBeNull();
  });

  it("retains unknown providers for their missing-provider diagnostic", () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    renderPanel(undefined, {
      panelModels: [
        {
          ...models[0],
          activeProviderId: "missing-provider",
          providerIds: ["missing-provider"],
        },
      ],
    });

    fireEvent.click(screen.getByRole("button", { name: /GPT-4.1/ }));

    expect(
      screen.getByRole("button", {
        name: /missing-provider.*Provider not found/,
      }),
    ).toBeDisabled();
  });

  it("resizes with mouse pointer events and persists the height ratio", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    const { container } = renderPanel();
    const availableHeight = 500;
    const startPanelHeight = 210;
    const { panel } = mockFlexibleStackLayout(container, {
      availableHeight,
      panelHeight: startPanelHeight,
    });

    // Force layout recompute with mocked rects.
    fireEvent(window, new Event("resize"));

    const splitter = await screen.findByRole("separator", {
      name: "Resize model routing panel",
    });

    await waitFor(() => {
      expect(panel.style.getPropertyValue("--model-routing-panel-height")).toBe(
        `${startPanelHeight}px`,
      );
    });

    fireEvent.pointerDown(splitter, {
      clientY: 300,
      pointerId: 1,
      pointerType: "mouse",
    });

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("row-resize");
      expect(document.body.style.userSelect).toBe("none");
    });

    // Drag up by 40px → panel grows by 40.
    fireEvent.pointerMove(window, { clientY: 260, pointerId: 1 });

    const expectedHeight = startPanelHeight + 40;
    await waitFor(() => {
      expect(panel.style.getPropertyValue("--model-routing-panel-height")).toBe(
        `${expectedHeight}px`,
      );
      expect(splitter).toHaveAttribute("aria-valuenow", String(expectedHeight));
    });

    fireEvent.pointerUp(window, { pointerId: 1 });

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("");
      expect(document.body.style.userSelect).toBe("");
    });

    const savedRatio = Number(
      window.localStorage.getItem(MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY),
    );
    expect(savedRatio).toBeCloseTo(expectedHeight / availableHeight, 5);
  });

  it("supports touch pointer drag and cleans up on pointercancel", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    const { container } = renderPanel();
    const availableHeight = 500;
    const startPanelHeight = 200;
    const { panel } = mockFlexibleStackLayout(container, {
      availableHeight,
      panelHeight: startPanelHeight,
    });
    fireEvent(window, new Event("resize"));

    const splitter = await screen.findByRole("separator", {
      name: "Resize model routing panel",
    });

    fireEvent.pointerDown(splitter, {
      clientY: 400,
      pointerId: 7,
      pointerType: "touch",
    });
    fireEvent.pointerMove(window, {
      clientY: 360,
      pointerId: 7,
      pointerType: "touch",
    });

    const expectedHeight = startPanelHeight + 40;
    await waitFor(() => {
      expect(panel.style.getPropertyValue("--model-routing-panel-height")).toBe(
        `${expectedHeight}px`,
      );
      expect(document.body.style.cursor).toBe("row-resize");
    });

    fireEvent.pointerCancel(window, { pointerId: 7, pointerType: "touch" });

    await waitFor(() => {
      expect(document.body.style.cursor).toBe("");
      expect(document.body.style.userSelect).toBe("");
    });

    // Last applied ratio is kept after cancel.
    expect(
      Number(window.localStorage.getItem(MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY)),
    ).toBeCloseTo(expectedHeight / availableHeight, 5);
    expect(panel.style.getPropertyValue("--model-routing-panel-height")).toBe(
      `${expectedHeight}px`,
    );
  });

  it("clamps drag against workspace-nav and model-routing mins", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    const { container } = renderPanel();
    const availableHeight = 500;
    const startPanelHeight = 200;
    const { panel } = mockFlexibleStackLayout(container, {
      availableHeight,
      panelHeight: startPanelHeight,
    });
    fireEvent(window, new Event("resize"));

    const splitter = await screen.findByRole("separator", {
      name: "Resize model routing panel",
    });

    fireEvent.pointerDown(splitter, { clientY: 300, pointerId: 1 });
    // Drag far up → hit nav min (max panel = available - navMin).
    fireEvent.pointerMove(window, { clientY: -500, pointerId: 1 });

    const maxPanel = availableHeight - WORKSPACE_NAV_MIN_HEIGHT_PX;
    await waitFor(() => {
      expect(panel.style.getPropertyValue("--model-routing-panel-height")).toBe(
        `${maxPanel}px`,
      );
    });

    // Drag far down → hit panel min.
    fireEvent.pointerMove(window, { clientY: 2000, pointerId: 1 });
    await waitFor(() => {
      expect(panel.style.getPropertyValue("--model-routing-panel-height")).toBe(
        `${MODEL_ROUTING_PANEL_MIN_HEIGHT_PX}px`,
      );
    });
    fireEvent.pointerUp(window, { pointerId: 1 });
  });

  it("resizes with keyboard arrows and updates aria values", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    const { container } = renderPanel();
    const availableHeight = 500;
    const startPanelHeight = panelHeightFromRatio(
      DEFAULT_MODEL_ROUTING_HEIGHT_RATIO,
      availableHeight,
    );
    const { panel } = mockFlexibleStackLayout(container, {
      availableHeight,
      panelHeight: startPanelHeight,
    });
    fireEvent(window, new Event("resize"));

    const splitter = await screen.findByRole("separator", {
      name: "Resize model routing panel",
    });

    await waitFor(() => {
      expect(splitter).toHaveAttribute(
        "aria-valuenow",
        String(startPanelHeight),
      );
    });

    fireEvent.keyDown(splitter, { key: "ArrowUp" });
    const upHeight = startPanelHeight + MODEL_ROUTING_RESIZE_STEP_PX;
    await waitFor(() => {
      expect(panel.style.getPropertyValue("--model-routing-panel-height")).toBe(
        `${upHeight}px`,
      );
      expect(splitter).toHaveAttribute("aria-valuenow", String(upHeight));
    });

    fireEvent.keyDown(splitter, { key: "ArrowDown" });
    await waitFor(() => {
      expect(panel.style.getPropertyValue("--model-routing-panel-height")).toBe(
        `${startPanelHeight}px`,
      );
    });
  });

  it("keeps the height ratio when collapsing and restores it on expand", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    window.localStorage.setItem(MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY, "0.5");
    const { container } = renderPanel();
    const availableHeight = 500;
    const { panel } = mockFlexibleStackLayout(container, {
      availableHeight,
      panelHeight: 250,
    });
    fireEvent(window, new Event("resize"));

    await waitFor(() => {
      expect(panel.style.getPropertyValue("--model-routing-panel-height")).toBe(
        "250px",
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Model routing" }));
    expect(
      screen.queryByRole("separator", { name: "Resize model routing panel" }),
    ).toBeNull();
    expect(window.localStorage.getItem(MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY)).toBe(
      "0.5",
    );
    expect(window.localStorage.getItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY)).toBe(
      "0",
    );

    fireEvent.click(screen.getByRole("button", { name: "Model routing" }));
    mockFlexibleStackLayout(container, {
      availableHeight,
      panelHeight: 250,
    });
    fireEvent(window, new Event("resize"));

    await waitFor(() => {
      expect(
        screen.getByRole("separator", { name: "Resize model routing panel" }),
      ).toBeTruthy();
      expect(
        (container.querySelector(".model-routing-panel") as HTMLElement).style
          .getPropertyValue("--model-routing-panel-height"),
      ).toBe("250px");
    });
    expect(window.localStorage.getItem(MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY)).toBe(
      "0.5",
    );
  });

  it("restores the saved ratio after remount and scales with a new stack height", async () => {
    window.localStorage.setItem(MODEL_ROUTING_EXPANDED_STORAGE_KEY, "1");
    window.localStorage.setItem(MODEL_ROUTING_HEIGHT_RATIO_STORAGE_KEY, "0.4");

    const first = renderPanel();
    mockFlexibleStackLayout(first.container, {
      availableHeight: 500,
      panelHeight: 200,
    });
    fireEvent(window, new Event("resize"));
    await waitFor(() => {
      expect(
        (first.container.querySelector(".model-routing-panel") as HTMLElement)
          .style.getPropertyValue("--model-routing-panel-height"),
      ).toBe("200px");
    });
    cleanup();

    const second = renderPanel();
    mockFlexibleStackLayout(second.container, {
      availableHeight: 800,
      panelHeight: 320,
    });
    fireEvent(window, new Event("resize"));
    await waitFor(() => {
      expect(
        (second.container.querySelector(".model-routing-panel") as HTMLElement)
          .style.getPropertyValue("--model-routing-panel-height"),
      ).toBe("320px");
    });
  });

  it("uses the default ratio when localStorage is unavailable", () => {
    const getItem = vi
      .spyOn(Storage.prototype, "getItem")
      .mockImplementation(() => {
        throw new Error("blocked");
      });
    expect(readModelRoutingHeightRatio()).toBe(DEFAULT_MODEL_ROUTING_HEIGHT_RATIO);
    getItem.mockRestore();
  });
});
