import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  activeMemory,
  appTestState,
  changeInput,
  chatMemory,
  chatMessages,
  deferred,
  enqueueChatStreamEvent,
  enqueueChatStreamEventForRun,
  jsonResponse,
  memoryDreamChange,
  memoryDreamJob,
  memoryExtractionJob,
  memorySource,
  mermaidMock,
  mockFetch,
  pendingMemory,
  renderApp,
  resetAppTestEnvironment,
  secondaryWorkspace,
  settings,
  todoGraph,
  workspace,
  workspaceMemory,
} from "./test-utils/app-test-harness";

describe("app-settings verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  it("shows settings sections for providers, models, MCP servers, and skills", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    expect(screen.getByRole("navigation", { name: "Foco" })).toBeInTheDocument();
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    const settingsSidebar = settingsNav.closest("aside");
    expect(settingsSidebar).not.toBeNull();
    expect(within(settingsSidebar as HTMLElement).getByText("Settings")).toBeInTheDocument();
    expect(await screen.findByText("General settings")).toBeInTheDocument();
    expect(screen.getByText("127.0.0.1:3210")).toBeInTheDocument();
    expect(screen.getByText("Password is disabled")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));
    expect(screen.getByText("Prompt settings")).toBeInTheDocument();
    expect(screen.getByText("Prompt files")).toBeInTheDocument();
    expect(screen.getByText("No prompt files")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    expect(screen.getByText("Configured providers")).toBeInTheDocument();
    const providersSection = screen.getByText("Configured providers").closest("section");
    expect(providersSection).not.toBeNull();
    expect(within(providersSection as HTMLElement).getByText("OpenAI")).toBeInTheDocument();
    await userEvent.click(
      within(providersSection as HTMLElement).getByRole("button", {
        name: "Load provider models for OpenAI",
      }),
    );
    expect(await within(providersSection as HTMLElement).findByText("gpt-4.1")).toBeInTheDocument();
    expect(within(providersSection as HTMLElement).getByText("gpt-4.1-mini")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Models" }));
    expect(screen.getByText("Model settings")).toBeInTheDocument();
    expect(screen.getByText("GPT Test")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "MCP" }));
    expect(screen.getByText("MCP servers")).toBeInTheDocument();
    expect(screen.getByText("CodeGraph")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));
    expect(screen.getByText("Detected skills")).toBeInTheDocument();
    expect(screen.getByText("Skill locations")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Refresh skill discovery" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Save skills" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Global skill")).toBeInTheDocument();
    expect(screen.getAllByText("gitmemo")).not.toHaveLength(0);
  });

  it("refreshes configured provider model support", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    const providersSection = screen.getByText("Configured providers").closest("section");
    expect(providersSection).not.toBeNull();

    await userEvent.click(
      within(providersSection as HTMLElement).getByRole("button", {
        name: "Refresh provider models",
      }),
    );

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/providers/models/refresh",
        expect.objectContaining({ method: "POST" }),
      ),
    );
    expect(await within(providersSection as HTMLElement).findByText("disabled")).toBeInTheDocument();

    const singleProviderFetchCount = fetchMock.mock.calls.filter(
      ([url]) => url === "/api/providers/models",
    ).length;
    await userEvent.click(
      within(providersSection as HTMLElement).getByRole("button", {
        name: "Load provider models for OpenAI",
      }),
    );

    expect(
      await within(providersSection as HTMLElement).findByText("gpt-4.1-refresh"),
    ).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(([url]) => url === "/api/providers/models"),
    ).toHaveLength(singleProviderFetchCount);
  });

  it("toggles the app theme from the nav rail", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(
      await screen.findByRole("button", { name: "Switch to dark theme" }),
    );

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining('"theme":"dark"'),
          method: "POST",
        }),
      );
    });
    await waitFor(() => {
      expect(document.documentElement.dataset.focoTheme).toBe("dark");
    });
  });

  it("saves the app theme from general settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.selectOptions(
      await screen.findByRole("combobox", { name: /Theme/ }),
      "dark",
    );

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining('"theme":"dark"'),
          method: "POST",
        }),
      );
    });
    await waitFor(() => {
      expect(document.documentElement.dataset.focoTheme).toBe("dark");
    });
  });

  it("saves Windows auto start from general settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(
      await screen.findByRole("checkbox", {
        name: "Start Foco when Windows starts",
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Save general settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining('"autoStartEnabled":true'),
          method: "POST",
        }),
      );
    });
  });

  it("saves API request audit settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const retentionInput = await screen.findByLabelText(
      "API request detail retention days",
    );
    await userEvent.clear(retentionInput);
    await userEvent.type(retentionInput, "7");
    await userEvent.click(
      screen.getByRole("checkbox", { name: "Save request and response bodies" }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Save general settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining(
            '"apiAudit":{"requestDetailRetentionDays":7,"saveRequestResponseDetails":false}',
          ),
          method: "POST",
        }),
      );
    });
  });

  it("saves memory settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Memory settings")).toBeInTheDocument();
    expect((await screen.findAllByText(activeMemory.fact)).length).toBeGreaterThan(0);

    await userEvent.click(screen.getByRole("button", { name: "Go to page 2" }));
    await waitFor(() => {
      const pageCall = [...fetchMock.mock.calls].find(([url]) => {
        const value = String(url);
        return (
          value.startsWith("/api/memory?") &&
          value.includes("page=2") &&
          value.includes("pageSize=20")
        );
      });
      expect(pageCall).toBeDefined();
    });

    await userEvent.click(screen.getByLabelText("Enable memory"));
    await userEvent.selectOptions(screen.getByLabelText("Extraction mode"), "automatic");
    await userEvent.selectOptions(screen.getByLabelText("Memory matching"), "llm");
    changeInput(screen.getByLabelText("Retention days"), "30");
    await userEvent.selectOptions(screen.getByLabelText("Extraction model"), "gpt-test");
    await userEvent.selectOptions(screen.getByLabelText("Matching model"), "gpt-test");
    await userEvent.click(screen.getByLabelText("Enable Dream"));
    await userEvent.click(screen.getByLabelText("Enable Auto Dream"));
    await userEvent.selectOptions(screen.getByLabelText("Dream mode"), "deterministic_only");
    await userEvent.selectOptions(screen.getByLabelText("Dream model"), "gpt-test");
    changeInput(screen.getByLabelText("Workspace interval days"), "5");
    changeInput(screen.getByLabelText("Global interval days"), "20");
    changeInput(screen.getByLabelText("Max facts per run"), "120");
    changeInput(screen.getByLabelText("Max changes per run"), "25");
    changeInput(screen.getByLabelText("Scheduler scan minutes"), "45");
    await userEvent.click(screen.getByLabelText("Create transcript chat"));
    await userEvent.click(screen.getByRole("button", { name: "Save memory settings" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/settings/memory",
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toEqual({
        enabled: true,
        extractionMode: "automatic",
        retrievalMode: "llm",
        extractionModelId: "gpt-test",
        retrievalModelId: "gpt-test",
        retentionDays: 30,
        dream: {
          enabled: true,
          autoEnabled: true,
          mode: "deterministic_only",
          modelId: "gpt-test",
          workspaceIntervalDays: 5,
          globalIntervalDays: 20,
          createTranscriptChat: false,
          maxFactsPerRun: 120,
          maxChangesPerRun: 25,
          schedulerScanMinutes: 45,
        },
      });
    });
  });

  it("shows Dream history details and runs manual Dream jobs", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Dream history")).toBeInTheDocument();
    expect(await screen.findByText(memoryDreamJob.summary!)).toBeInTheDocument();
    expect(await screen.findByText(memoryDreamChange.reason)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open transcript" })).toBeInTheDocument();

    await userEvent.click(screen.getByLabelText("Enable memory"));
    await userEvent.click(screen.getByLabelText("Enable Dream"));
    await userEvent.click(screen.getByRole("button", { name: "Run workspace Dream now" }));
    await userEvent.click(screen.getByRole("button", { name: "Run global Dream now" }));

    await waitFor(() => {
      const dreamRunCalls = fetchMock.mock.calls.filter(
        ([url]) => url === "/api/memory/dream/run",
      );
      expect(dreamRunCalls).toHaveLength(2);
      expect(JSON.parse(String(dreamRunCalls[0]?.[1]?.body))).toEqual({
        scope: "workspace",
        workspaceId: "workspace-1",
        triggerType: "manual",
        mode: "llm",
      });
      expect(JSON.parse(String(dreamRunCalls[1]?.[1]?.body))).toEqual({
        scope: "global",
        triggerType: "manual",
        mode: "llm",
      });
    });
  });

  it("creates and edits manual memories", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Memory settings")).toBeInTheDocument();
    expect((await screen.findAllByText(activeMemory.fact)).length).toBeGreaterThan(0);

    await userEvent.click(screen.getByRole("button", { name: "Create memory" }));
    const createDialog = await screen.findByRole("dialog", { name: "Create memory" });
    changeInput(
      within(createDialog).getByLabelText("Memory fact"),
      "Remember local memory graph.",
    );
    await userEvent.click(within(createDialog).getByRole("button", { name: "Create memory" }));

    await waitFor(() => {
      const createCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/manual",
      );
      expect(createCall).toBeDefined();
      expect(JSON.parse(String(createCall?.[1]?.body))).toEqual({
        chatId: null,
        confidence: null,
        fact: "Remember local memory graph.",
        kind: "user_note",
        metadata: {},
        pinned: false,
        scope: "global",
        workspaceId: null,
      });
    });

    const editButtons = screen.getAllByRole("button", { name: "Edit memory" });
    await userEvent.click(editButtons[0]);
    const editDialog = await screen.findByRole("dialog", { name: "Edit memory" });
    expect(within(editDialog).getByText("Memory details")).toBeInTheDocument();
    expect(await within(editDialog).findAllByText("Expand JSON")).toHaveLength(2);
    await userEvent.click(
      within(editDialog).getByRole("button", { name: "Expand JSON Source content" }),
    );
    expect(within(editDialog).getAllByLabelText("Source content")).toHaveLength(1);
    expect(within(editDialog).getAllByText(/"origin"/).length).toBeGreaterThan(0);
    changeInput(within(editDialog).getByLabelText("Memory fact"), "Updated memory preference.");
    await userEvent.click(within(editDialog).getByRole("button", { name: "Save memory" }));

    await waitFor(() => {
      const editCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/edit",
      );
      expect(editCall).toBeDefined();
      expect(JSON.parse(String(editCall?.[1]?.body))).toEqual({
        confidence: null,
        fact: "Updated memory preference.",
        kind: "preference",
        metadata: {},
        memoryId: activeMemory.id,
        pinned: true,
        scope: "global",
        sources: [
          {
            content: memorySource.content,
            id: memorySource.id,
            metadata: { source: "manual" },
            title: memorySource.title,
          },
        ],
        workspaceId: null,
      });
    });
  });

  it("filters, clears, and promotes workspace memories", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Memory settings")).toBeInTheDocument();
    expect((await screen.findAllByText(activeMemory.fact)).length).toBeGreaterThan(0);

    await userEvent.selectOptions(screen.getByLabelText("Memory scope"), "workspace");
    expect(await screen.findByText(workspaceMemory.fact)).toBeInTheDocument();
    await userEvent.selectOptions(screen.getByLabelText("Memory kind"), "preference");
    await waitFor(() => {
      const filteredListCall = [...fetchMock.mock.calls].find(([url]) =>
        String(url).startsWith("/api/memory?") &&
        String(url).includes("scope=workspace") &&
        String(url).includes("kind=preference"),
      );
      expect(filteredListCall).toBeDefined();
    });

    await userEvent.click(
      screen.getByRole("button", { name: "Clear filtered workspace memories" }),
    );

    await waitFor(() => {
      const clearCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/clear",
      );
      expect(clearCall).toBeDefined();
      expect(JSON.parse(String(clearCall?.[1]?.body))).toEqual({
        chatId: null,
        kind: "preference",
        query: null,
        scope: "workspace",
        status: "active",
        workspaceId: workspace.id,
      });
    });

    await userEvent.click(screen.getByRole("button", { name: "Promote one level" }));

    await waitFor(() => {
      const promoteCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/promote",
      );
      expect(promoteCall).toBeDefined();
      expect(JSON.parse(String(promoteCall?.[1]?.body))).toEqual({
        memoryId: workspaceMemory.id,
        scope: "workspace",
        targetChatId: null,
        targetScope: "global",
        targetWorkspaceId: null,
        workspaceId: workspace.id,
      });
    });
    confirmSpy.mockRestore();
  });

  it("deletes and approves memories", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Memory settings")).toBeInTheDocument();
    expect((await screen.findAllByText(activeMemory.fact)).length).toBeGreaterThan(0);

    await userEvent.click(screen.getAllByRole("button", { name: "Delete memory" })[0]);
    await waitFor(() => {
      expect(confirmSpy).toHaveBeenCalledWith("Delete memory confirmation");
      const forgetCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/forget",
      );
      expect(forgetCall).toBeDefined();
      expect(JSON.parse(String(forgetCall?.[1]?.body))).toEqual({
        memoryId: activeMemory.id,
        scope: "global",
        workspaceId: null,
      });
    });

    await userEvent.selectOptions(screen.getByLabelText("Memory status"), "pending");
    expect((await screen.findAllByText(pendingMemory.fact)).length).toBeGreaterThan(0);
    await userEvent.click(screen.getByRole("button", { name: "Approve memory" }));

    await waitFor(() => {
      const statusCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/status",
      );
      expect(statusCall).toBeDefined();
      expect(JSON.parse(String(statusCall?.[1]?.body))).toEqual({
        memoryId: pendingMemory.id,
        scope: "global",
        status: "active",
        workspaceId: null,
      });
    });
    confirmSpy.mockRestore();
  });

  it("keeps chat memory pagination requests tied to a chat id", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));
    expect(await screen.findByText("Memory settings")).toBeInTheDocument();

    const callCountBeforeChatScope = fetchMock.mock.calls.length;
    await userEvent.selectOptions(screen.getByLabelText("Memory scope"), "chat");

    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Go to page 2" })).not.toBeInTheDocument();
    });
    const missingChatIdCall = fetchMock.mock.calls
      .slice(callCountBeforeChatScope)
      .find(([url]) => {
        const value = String(url);
        return value.startsWith("/api/memory?") && value.includes("scope=chat");
      });
    expect(missingChatIdCall).toBeUndefined();

    await userEvent.type(screen.getByLabelText("Chat ID"), "chat-test");
    expect(await screen.findByText(chatMemory.fact)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Go to page 2" }));

    await waitFor(() => {
      const pageCall = [...fetchMock.mock.calls].find(([url]) => {
        const value = String(url);
        return (
          value.startsWith("/api/memory?") &&
          value.includes("scope=chat") &&
          value.includes("chatId=chat-test") &&
          value.includes("page=2")
        );
      });
      expect(pageCall).toBeDefined();
    });
  });

  it("shows translated hook settings and imports Claude hooks by target scope", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Hooks" }));

    expect(await screen.findByText("Hook settings")).toBeInTheDocument();
    expect(screen.getAllByText("Pre tool use").length).toBeGreaterThan(0);
    expect(screen.getAllByText("User prompt submit").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Command").length).toBeGreaterThan(0);
    expect(screen.getAllByText("HTTP").length).toBeGreaterThan(0);
    expect(screen.getByText("Record hook run logs")).toBeInTheDocument();
    expect(
      screen.getByText("Global import reads user Claude settings; workspace import reads the selected workspace."),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Import to global hooks" }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/hooks/import-claude",
        expect.objectContaining({
          body: JSON.stringify({ target: "global", workspaceId: null }),
          method: "POST",
        }),
      );
    });

    await userEvent.click(screen.getByRole("button", { name: "Import to workspace hooks" }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/hooks/import-claude",
        expect.objectContaining({
          body: JSON.stringify({ target: "workspace", workspaceId: "workspace-1" }),
          method: "POST",
        }),
      );
    });

    await userEvent.click(screen.getByRole("button", { name: /Pre tool use/ }));
    const dialog = await screen.findByRole("dialog", { name: "Hook run detail" });
    expect(dialog).toBeInTheDocument();
    expect(within(dialog).getByText("succeeded")).toBeInTheDocument();
  });

  it("logs in before loading the browser UI when authentication is enabled", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];

      if (path === "/api/auth/status") {
        return jsonResponse({ authenticated: false, enabled: true });
      }

      if (path === "/api/auth/login") {
        expect(init?.body).toBe(JSON.stringify({ password: "secret" }));
        return jsonResponse({ authenticated: true, enabled: true });
      }

      return mockFetch(input);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    expect(await screen.findByText("Password required")).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText("Password"), "secret");
    await userEvent.click(screen.getByRole("button", { name: "Log in" }));

    expect(await screen.findByText("Tool run")).toBeInTheDocument();
  });

  it("saves browser authentication password from general settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const passwordInput = await screen.findByLabelText("Authentication password");
    expect(passwordInput).toHaveAttribute("type", "password");
    expect(screen.queryByRole("button", { name: "Log out" })).not.toBeInTheDocument();

    await userEvent.type(passwordInput, "secret");
    await userEvent.click(screen.getByRole("button", { name: "Show password" }));
    expect(passwordInput).toHaveAttribute("type", "text");
    expect(screen.queryByRole("checkbox", { name: "Clear browser password" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear browser password" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Save general settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining('"password":"secret"'),
          method: "POST",
        }),
      );
    });
    await waitFor(() => {
      expect(passwordInput).toHaveValue("********");
    });
    expect(screen.getByRole("button", { name: "Show password" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Log out" })).toBeInTheDocument();

    await userEvent.click(passwordInput);
    await userEvent.type(passwordInput, "replacement");
    await userEvent.click(screen.getByRole("button", { name: "Show password" }));
    expect(passwordInput).toHaveAttribute("type", "text");
    expect(passwordInput).toHaveValue("replacement");
  });

  it("saves prompt files and extra prompt text", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));

    const systemPromptInput = screen.getByLabelText("System prompt");
    expect(systemPromptInput).toHaveValue("You are Foco, a local coding agent.");
    await userEvent.clear(systemPromptInput);
    await userEvent.type(systemPromptInput, "Custom system prompt.");
    await userEvent.type(screen.getByPlaceholderText("Prompt name"), "Review");
    await userEvent.click(screen.getByRole("button", { name: "Add system prompt" }));
    expect(screen.getAllByText("Review").length).toBeGreaterThan(0);
    await userEvent.type(screen.getByLabelText("System prompt"), "Review as senior engineer.");
    await userEvent.type(
      screen.getByLabelText("Prompt file path"),
      "C:/Users/fonla/.codex/AGENTS.md",
    );
    await userEvent.click(screen.getByRole("button", { name: "Add prompt file" }));
    await userEvent.type(screen.getByLabelText("Extra prompt"), "Keep replies concise.");
    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/prompts",
        expect.objectContaining({
          body: JSON.stringify({
            extraText: "Keep replies concise.",
            files: ["C:/Users/fonla/.codex/AGENTS.md"],
            systemPrompts: [
              {
                content: "Custom system prompt.",
                name: "Default",
              },
              {
                name: "Review",
                content: "Review as senior engineer.",
              },
            ],
          }),
          method: "POST",
        }),
      );
    });
  });

  it("restores the default system prompt", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));

    const systemPromptInput = screen.getByLabelText("System prompt");
    await userEvent.clear(systemPromptInput);
    await userEvent.type(systemPromptInput, "Custom system prompt.");
    await userEvent.click(screen.getByRole("button", { name: "Restore default system prompt" }));
    expect(systemPromptInput).toHaveValue("You are Foco, a local coding agent.");

    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/prompts",
        expect.objectContaining({
          body: JSON.stringify({
            extraText: "",
            files: [],
            systemPrompts: [
              {
                content: "You are Foco, a local coding agent.",
                name: "Default",
              },
            ],
          }),
          method: "POST",
        }),
      );
    });
  });

  it("closes the model dialog from the backdrop without saving", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Add model" }));

    expect(
      await screen.findByRole("form", { name: "Model configuration" }),
    ).toBeInTheDocument();

    await userEvent.click(
      screen.getByRole("button", { name: "Close model configuration backdrop" }),
    );

    await waitFor(() => {
      expect(
        screen.queryByRole("form", { name: "Model configuration" }),
      ).not.toBeInTheDocument();
    });
    expect(fetchMock.mock.calls.some(([url]) => url === "/api/models/manual")).toBe(
      false,
    );
  });

  it("saves provider, model, MCP server, and skill settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);

    await userEvent.click(screen.getByRole("button", { name: "Providers" }));
    await userEvent.click(screen.getByRole("button", { name: "Edit provider OpenAI" }));
    const providerApiKeyInput = screen.getByLabelText("API key");
    await userEvent.type(providerApiKeyInput, "sk-visible");
    const showApiKeyButton = screen.getByRole("button", { name: "Show API key" });
    const clearApiKeyButton = screen.getByRole("button", { name: "Clear saved API key" });
    expect(
      Boolean(
        showApiKeyButton.compareDocumentPosition(clearApiKeyButton) &
        Node.DOCUMENT_POSITION_FOLLOWING,
      ),
    ).toBe(true);
    await userEvent.click(showApiKeyButton);
    expect(providerApiKeyInput).toHaveAttribute("type", "text");
    await userEvent.click(screen.getByRole("button", { name: "Close provider configuration" }));

    await userEvent.click(screen.getByRole("button", { name: "Add provider" }));
    expect(screen.getByLabelText("Protocol")).toHaveValue("openai-responses");
    await userEvent.type(screen.getByLabelText("Name"), "Test Provider");
    await userEvent.click(screen.getByRole("checkbox", { name: "Enable AI API proxy" }));
    await userEvent.selectOptions(screen.getByLabelText("Proxy type"), "socks");
    await userEvent.type(screen.getByLabelText("Proxy server"), "127.0.0.1:7891");
    await userEvent.click(screen.getByRole("button", { name: "Save provider" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/providers/manual",
        expect.objectContaining({
          body: expect.stringContaining('"name":"Test Provider"'),
          method: "POST",
        }),
      );
    });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/providers/manual",
      expect.objectContaining({
        body: expect.stringContaining(
          '"apiProxy":{"enabled":true,"proxyType":"socks","url":"127.0.0.1:7891"}',
        ),
        method: "POST",
      }),
    );
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/providers/manual",
      expect.objectContaining({
        body: expect.stringContaining('"kind":"openai-responses"'),
        method: "POST",
      }),
    );

    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Add model" }));
    await userEvent.type(screen.getByLabelText("Model id"), "created-model");
    await userEvent.type(screen.getByLabelText("Display name"), "Created Model");
    await userEvent.type(screen.getByLabelText("Context window"), "32000");
    await userEvent.type(screen.getByLabelText("Max output tokens"), "2048");
    await userEvent.click(screen.getByRole("button", { name: "Save model" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/models/manual",
        expect.objectContaining({
          body: expect.stringContaining('"modelId":"created-model"'),
          method: "POST",
        }),
      );
    });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/models/manual",
      expect.objectContaining({
        body: expect.stringContaining('"systemPromptName":"Default"'),
        method: "POST",
      }),
    );

    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "MCP" }));
    await userEvent.click(screen.getByRole("button", { name: "Add MCP server" }));
    await userEvent.type(screen.getByLabelText("Name"), "Test MCP");
    await userEvent.type(screen.getByLabelText("Command"), "foco-test-mcp");
    await userEvent.click(screen.getByRole("button", { name: "Save MCP server" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/mcp/servers/manual",
        expect.objectContaining({
          body: expect.stringContaining('"name":"Test MCP"'),
          method: "POST",
        }),
      );
    });

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));
    await userEvent.click(screen.getByLabelText("Enable skill gitmemo"));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/skills/manual",
        expect.objectContaining({
          body: JSON.stringify({
            disabled: [],
            enabled: ["global:gitmemo"],
          }),
          method: "POST",
        }),
      );
    });
  }, 10000);

});
