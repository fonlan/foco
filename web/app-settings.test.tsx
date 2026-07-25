import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEventApi from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
  ConfiguredSkillSummary,
  MemoryDreamJobSummary,
  RemoteServerSummary,
} from "./api/types";
import {
  activeMemory,
  agentDefinitions,
  appTestState,
  changeInput,
  chatMemory,
  chatMessages,
  deferred,
  enqueueChatStreamEvent,
  enqueueChatStreamEventForRun,
  failedMemoryDreamJob,
  jsonResponse,
  memoryDreamChange,
  memoryDreamJob,
  memoryExtractionJob,
  memorySource,
  mermaidMock,
  mockFetch,
  pendingMemory,
  defaultPlanModeSystemPrompt,
  defaultReviewSystemPrompt,
  renderApp,
  resetAppTestEnvironment,
  secondaryWorkspace,
  settings,
  todoGraph,
  workspace,
  workspaceMemory,
} from "./test-utils/app-test-harness";
import { installUpdateAndWaitForRestart } from "./shared/update-install";

/**
 * Settings selectors are now HeroUI ListBoxes rather than native <select>s.
 * Preserve the terse existing test call sites while exercising the same
 * trigger-and-option interaction a keyboard or pointer user performs.
 */
async function selectOptions(
  control: Parameters<typeof userEventApi.selectOptions>[0],
  values: Parameters<typeof userEventApi.selectOptions>[1],
) {
  if (control instanceof HTMLSelectElement) {
    return userEventApi.selectOptions(control, values);
  }

  const requested = Array.isArray(values) ? values : [values];
  await userEventApi.click(control);

  for (const value of requested) {
    const options = await screen.findAllByRole("option");
    const option = options.find((candidate) => candidate.getAttribute("data-key") === value);
    if (!option) {
      throw new Error(`HeroUI option with key ${value} was not available.`);
    }
    await userEventApi.click(option);
  }
}

async function expectSelectedOption(control: HTMLElement, value: string) {
  await userEventApi.click(control);
  const options = await screen.findAllByRole("option");
  const selected = options.find((candidate) => candidate.getAttribute("data-key") === value);
  expect(selected).toHaveAttribute("aria-selected", "true");
  await userEventApi.keyboard("{Escape}");
}

const userEvent = { ...userEventApi, selectOptions };

describe("app-settings verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  it("uses the right settings column as the only primary scroll container", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    const settingsShell = settingsNav.closest(".settings-shell");
    const settingsContent = screen.getByText("General settings").closest(".settings-content-scroll");

    expect(settingsShell).not.toBeNull();
    expect(settingsShell).not.toHaveClass("panel-scroll");
    expect(settingsShell).not.toHaveClass("overflow-y-auto");
    expect(settingsContent).not.toBeNull();
    expect(settingsContent).toHaveClass("panel-scroll");
    expect(settingsShell?.querySelectorAll(".settings-content-scroll.panel-scroll")).toHaveLength(1);
    expect(settingsContent?.closest(".settings-shell")).toBe(settingsShell);
  });

  it("shows settings sections for providers, models, MCP servers, and skills", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    expect(screen.getByRole("navigation", { name: "Foco" })).toBeInTheDocument();
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    const settingsNavButtons = within(settingsNav).getAllByRole("button");
    expect(settingsNavButtons.at(-1)).toHaveAccessibleName("About");
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

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));
    expect(screen.getByText("Spec settings")).toBeInTheDocument();
    expect(screen.getByText("Auto Spec")).toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    expect(screen.getByText("Configured providers")).toBeInTheDocument();
    const providersSection = screen.getByText("Configured providers").closest("section");
    expect(providersSection).not.toBeNull();
    expect(within(providersSection as HTMLElement).getByText("OpenAI")).toBeInTheDocument();
    expect(within(providersSection as HTMLElement).getByText("auto sync")).toBeInTheDocument();
    expect(
      within(providersSection as HTMLElement).getByText("sync regex ^gpt-4"),
    ).toBeInTheDocument();
    await userEvent.click(
      within(providersSection as HTMLElement).getByRole("button", {
        name: "Load provider models for OpenAI",
      }),
    );
    expect(await within(providersSection as HTMLElement).findByText("gpt-4.1")).toBeInTheDocument();
    expect(within(providersSection as HTMLElement).queryByText("gpt-4.1-mini")).toBeNull();

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
    await waitFor(() => {
      expect(screen.getByRole("checkbox", { name: "Enable skill gitmemo" })).toBeChecked();
    });
    expect(screen.getAllByText("gitmemo")).not.toHaveLength(0);
    expect(screen.queryByRole("button", { name: "Update all store skills" })).toBeNull();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "About" }));
    expect(screen.getByText("About Foco")).toBeInTheDocument();
    expect(screen.getAllByText("0.1.8").length).toBeGreaterThan(0);
    const githubLink = screen.getByRole("link", {
      name: "Open GitHub repository",
    });
    expect(githubLink).toHaveAttribute("href", "https://github.com/fonlan/foco");
    expect(githubLink).toHaveAttribute("target", "_blank");
    expect(githubLink).toHaveAttribute("rel", "noreferrer");
  });

  it("lazily aggregates matching remote workspace skills with remote controls", async () => {
    const fetchMock = vi.mocked(fetch);
    const remoteWorkspace = {
      ...appTestState.settingsResponse.workspaces[0]!,
      connectionStatus: "connected",
      id: "workspace-remote-1",
      isDefault: false,
      name: "Remote project",
      remotePath: "/srv/project",
      serverId: "server-1",
      serverName: "build-box",
    };
    const remoteSkill: ConfiguredSkillSummary = {
      ...appTestState.settingsResponse.skills.detected[0]!,
      canEnable: true,
      description: "Deploy the remote project.",
      id: "deploy",
      key: "workspace:workspace-remote-1:deploy",
      name: "Remote deploy",
      path: "/srv/project/.agents/skills/deploy/SKILL.md",
      scope: "workspace",
      warnings: ["The remote Skill requires a newer runtime."],
      workspaceId: remoteWorkspace.id,
      workspaceName: "Incorrect response name",
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      workspaces: [...appTestState.settingsResponse.workspaces, remoteWorkspace],
    };
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [remoteWorkspace.id]: {
        skills: [
          { ...remoteSkill, scope: "global", workspaceId: null },
          { ...remoteSkill, key: "workspace:other:deploy", workspaceId: "other" },
          remoteSkill,
        ],
      },
    };

    renderApp();
    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);

    expect(
      fetchMock.mock.calls.filter(([url]) =>
        String(url).includes("/api/workspaces/workspace-remote-1/skills"),
      ),
    ).toHaveLength(0);

    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));

    expect(await screen.findByText("Remote deploy")).toBeInTheDocument();
    expect(screen.getByText("The remote Skill requires a newer runtime.")).toBeInTheDocument();
    expect(screen.queryByText("Incorrect response name")).toBeNull();
    expect(screen.queryByText("Remote workspace")).toBeNull();
    expect(screen.queryByText("Read-only")).toBeNull();
    expect(screen.queryByText("Can enable")).toBeNull();
    expect(screen.queryByText("Remote project · build-box")).toBeNull();
    const remoteToggle = screen.getByRole("checkbox", { name: "Enable skill Remote deploy" });
    expect(remoteToggle).toBeEnabled();
    expect(screen.getByRole("button", { name: "Delete skill Remote deploy" })).toBeEnabled();
    expect(screen.queryByRole("button", { name: "Update skill Remote deploy" })).toBeNull();
    await userEvent.click(remoteToggle);
    await waitFor(() => {
      const call = fetchMock.mock.calls.find(
        ([url, init]) =>
          url === "/api/workspaces/workspace-remote-1/skills/manual" && init?.method === "POST",
      );
      expect(call).toBeDefined();
      expect(JSON.parse(String(call?.[1]?.body))).toEqual({
        enabled: false,
        key: remoteSkill.key,
      });
    });

    vi.spyOn(window, "confirm").mockReturnValueOnce(true);
    await userEvent.click(screen.getByRole("button", { name: "Delete skill Remote deploy" }));
    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url, init]) =>
            url === "/api/workspaces/workspace-remote-1/skills/delete" && init?.method === "POST",
        ),
      ).toBe(true);
    });
    expect(
      fetchMock.mock.calls.filter(([url]) =>
        String(url).includes("/api/workspaces/workspace-remote-1/skills"),
      ),
    ).not.toHaveLength(0);
  });

  it("keeps same-named remote workspace skills distinct while filtering remote global entries", async () => {
    const firstWorkspace = {
      ...appTestState.settingsResponse.workspaces[0]!,
      connectionStatus: "connected",
      id: "workspace-remote-first",
      isDefault: false,
      name: "First remote",
      remotePath: "/srv/first",
      serverId: "server-first",
      serverName: "first-host",
    };
    const secondWorkspace = {
      ...firstWorkspace,
      id: "workspace-remote-second",
      name: "Second remote",
      remotePath: "/srv/second",
      serverId: "server-second",
      serverName: "second-host",
    };
    const remoteGlobalSkill: ConfiguredSkillSummary = {
      ...appTestState.settingsResponse.skills.detected[0]!,
      id: "shared",
      key: "global:shared",
      name: "Shared remote skill",
      path: "/home/foco/.agents/skills/shared/SKILL.md",
      scope: "global",
      workspaceId: null,
      workspaceName: null,
    };
    const firstWorkspaceSkill: ConfiguredSkillSummary = {
      ...remoteGlobalSkill,
      key: "workspace:workspace-remote-first:shared",
      path: "/srv/first/.agents/skills/shared/SKILL.md",
      scope: "workspace",
      workspaceId: firstWorkspace.id,
      workspaceName: firstWorkspace.name,
    };
    const secondWorkspaceSkill: ConfiguredSkillSummary = {
      ...firstWorkspaceSkill,
      key: "workspace:workspace-remote-second:shared",
      path: "/srv/second/.agents/skills/shared/SKILL.md",
      workspaceId: secondWorkspace.id,
      workspaceName: secondWorkspace.name,
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      workspaces: [
        ...appTestState.settingsResponse.workspaces,
        firstWorkspace,
        secondWorkspace,
      ],
    };
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [firstWorkspace.id]: {
        skills: [remoteGlobalSkill, firstWorkspaceSkill],
      },
      [secondWorkspace.id]: {
        skills: [remoteGlobalSkill, secondWorkspaceSkill],
      },
    };

    renderApp();
    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));

    expect(await screen.findAllByText("Shared remote skill")).toHaveLength(2);
    expect(screen.getByText("skills 3")).toBeInTheDocument();
    expect(screen.getAllByRole("checkbox", { name: "Enable skill Shared remote skill" })).toHaveLength(2);
    expect(screen.queryByText("First remote · first-host")).toBeNull();
    expect(screen.queryByText("Second remote · second-host")).toBeNull();
    expect(screen.queryByText("/home/foco/.agents/skills/shared/SKILL.md")).toBeNull();
  });

  it("keeps a disconnected remote workspace isolated until the user retries discovery", async () => {
    const fetchMock = vi.mocked(fetch);
    const remoteWorkspace = {
      ...appTestState.settingsResponse.workspaces[0]!,
      connectionStatus: "offline",
      id: "workspace-remote-offline",
      isDefault: false,
      lastRemoteError: "Remote connection is offline",
      name: "Offline remote",
      remotePath: "/srv/offline",
      serverId: "server-offline",
      serverName: "offline-host",
    };
    const remoteSkill: ConfiguredSkillSummary = {
      ...appTestState.settingsResponse.skills.detected[0]!,
      id: "retried-skill",
      key: "workspace:workspace-remote-offline:retried-skill",
      name: "Retried remote skill",
      path: "/srv/offline/.agents/skills/retried-skill/SKILL.md",
      scope: "workspace",
      workspaceId: remoteWorkspace.id,
      workspaceName: remoteWorkspace.name,
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      workspaces: [...appTestState.settingsResponse.workspaces, remoteWorkspace],
    };
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [remoteWorkspace.id]: { skills: [remoteSkill] },
    };

    renderApp();
    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Offline remote · offline-host: Remote connection is offline",
    );
    expect(
      fetchMock.mock.calls.filter(([url]) =>
        String(url).includes("/api/workspaces/workspace-remote-offline/skills"),
      ),
    ).toHaveLength(0);

    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Retried remote skill")).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(([url]) =>
        String(url).includes("/api/workspaces/workspace-remote-offline/skills"),
      ),
    ).toHaveLength(1);
  });

  it("keeps other remote catalogs visible when one discovery fails and retries that workspace", async () => {
    const readyWorkspace = {
      ...appTestState.settingsResponse.workspaces[0]!,
      connectionStatus: "connected",
      id: "workspace-remote-ready",
      isDefault: false,
      name: "Ready remote",
      remotePath: "/srv/ready",
      serverId: "server-ready",
      serverName: "ready-host",
    };
    const failingWorkspace = {
      ...readyWorkspace,
      id: "workspace-remote-failing",
      name: "Failing remote",
      remotePath: "/srv/failing",
      serverId: "server-failing",
      serverName: "failing-host",
    };
    const readySkill: ConfiguredSkillSummary = {
      ...appTestState.settingsResponse.skills.detected[0]!,
      id: "ready-skill",
      key: "workspace:workspace-remote-ready:ready-skill",
      name: "Ready remote skill",
      path: "/srv/ready/.agents/skills/ready-skill/SKILL.md",
      scope: "workspace",
      workspaceId: readyWorkspace.id,
      workspaceName: readyWorkspace.name,
    };
    const recoveredSkill: ConfiguredSkillSummary = {
      ...readySkill,
      id: "recovered-skill",
      key: "workspace:workspace-remote-failing:recovered-skill",
      name: "Recovered remote skill",
      path: "/srv/failing/.agents/skills/recovered-skill/SKILL.md",
      workspaceId: failingWorkspace.id,
      workspaceName: failingWorkspace.name,
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      workspaces: [
        ...appTestState.settingsResponse.workspaces,
        readyWorkspace,
        failingWorkspace,
      ],
    };
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [readyWorkspace.id]: { skills: [readySkill] },
      [failingWorkspace.id]: [
        jsonResponse({ error: "Sidecar unavailable" }, { status: 502 }),
        jsonResponse({ error: "Sidecar unavailable" }, { status: 502 }),
      ],
    };

    renderApp();
    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));

    expect(await screen.findByText("Ready remote skill")).toBeInTheDocument();
    const discoveryError = await screen.findByRole("alert");
    expect(discoveryError).toHaveTextContent("Failing remote · failing-host: Sidecar unavailable");
    appTestState.workspaceSkillsResponsesByWorkspaceId[failingWorkspace.id] = {
      skills: [recoveredSkill],
    };
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Recovered remote skill")).toBeInTheDocument();
    expect(screen.getByText("Ready remote skill")).toBeInTheDocument();
  });

  it("refreshes remote workspace catalogs and ignores responses from the prior generation", async () => {
    const fetchMock = vi.mocked(fetch);
    const staleResponse = deferred<Response>();
    const remoteWorkspace = {
      ...appTestState.settingsResponse.workspaces[0]!,
      connectionStatus: "connected",
      id: "workspace-remote-race",
      isDefault: false,
      name: "Race remote",
      remotePath: "/srv/race",
      serverId: "server-race",
      serverName: "race-host",
    };
    const staleSkill: ConfiguredSkillSummary = {
      ...appTestState.settingsResponse.skills.detected[0]!,
      id: "stale-skill",
      key: "workspace:workspace-remote-race:stale-skill",
      name: "Stale remote skill",
      path: "/srv/race/.agents/skills/stale-skill/SKILL.md",
      scope: "workspace",
      workspaceId: remoteWorkspace.id,
      workspaceName: remoteWorkspace.name,
    };
    const freshSkill: ConfiguredSkillSummary = {
      ...staleSkill,
      id: "fresh-skill",
      key: "workspace:workspace-remote-race:fresh-skill",
      name: "Fresh remote skill",
      path: "/srv/race/.agents/skills/fresh-skill/SKILL.md",
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      workspaces: [...appTestState.settingsResponse.workspaces, remoteWorkspace],
    };
    appTestState.workspaceSkillsResponsesByWorkspaceId = {
      [remoteWorkspace.id]: [
        staleResponse.promise,
        jsonResponse({ skills: [freshSkill], errors: [] }),
      ],
    };

    renderApp();
    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));

    await userEvent.click(screen.getByRole("button", { name: "Refresh skill discovery" }));
    expect(await screen.findByText("Fresh remote skill")).toBeInTheDocument();
    staleResponse.resolve(jsonResponse({ skills: [staleSkill], errors: [] }));
    await act(async () => undefined);

    expect(screen.queryByText("Stale remote skill")).toBeNull();
    expect(
      fetchMock.mock.calls.filter(([url]) =>
        String(url).includes("/api/workspaces/workspace-remote-race/skills"),
      ).length,
    ).toBeGreaterThanOrEqual(2);
  });

  it("shows the five-minute default for Spec and every Memory LLM request", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));
    expect(
      (await screen.findByLabelText("Spec LLM timeout ms") as HTMLInputElement).value,
    ).toBe("300000");

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));
    expect(
      (await screen.findByLabelText("Retrieval LLM timeout ms") as HTMLInputElement).value,
    ).toBe("300000");
    expect(
      (screen.getByLabelText("Extraction LLM timeout ms") as HTMLInputElement).value,
    ).toBe("300000");
    expect(
      (screen.getByLabelText("Dream LLM timeout ms") as HTMLInputElement).value,
    ).toBe("300000");
  });

  it("tests configured models with per-row loading and toast feedback", async () => {
    const fetchMock = vi.mocked(fetch);
    const pendingResponse = deferred<Response>();
    const secondModel: ConfiguredModelSummary = {
      ...appTestState.settingsResponse.configuredModels[0]!,
      activeProviderId: "anthropic",
      displayName: "Claude Test",
      id: "claude-test",
      providerIds: ["anthropic"],
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      configuredModels: [...appTestState.settingsResponse.configuredModels, secondModel],
    };
    appTestState.modelTestResponses.push(
      pendingResponse.promise,
      jsonResponse({
        message: "Image output models cannot be tested with a text request",
        modelId: "gpt-test",
        ok: false,
        providerId: "openai",
      }),
    );
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Models" }));

    const testButton = screen.getByRole("button", { name: "Test model GPT Test" });
    const secondTestButton = screen.getByRole("button", { name: "Test model Claude Test" });
    expect(testButton).toHaveAccessibleName("Test model GPT Test");
    expect(testButton).toBeEnabled();
    expect(secondTestButton).toBeEnabled();

    await userEvent.click(testButton);
    expect(testButton).toBeDisabled();
    expect(secondTestButton).toBeEnabled();
    expect(testButton.querySelector(".animate-spin")).not.toBeNull();

    await userEvent.click(testButton);
    expect(fetchMock.mock.calls.filter(([url]) => url === "/api/models/test")).toHaveLength(1);

    pendingResponse.resolve(
      jsonResponse({
        message: "Model responded successfully",
        modelId: "gpt-test",
        ok: true,
        providerId: "openai",
      }),
    );

    const successToast = await screen.findByRole("status");
    expect(successToast).toHaveTextContent("Model test succeeded for GPT Test: Model responded successfully");
    await waitFor(() => expect(testButton).toBeEnabled());
    expect(testButton.querySelector(".animate-spin")).toBeNull();
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/models/test",
      expect.objectContaining({
        body: JSON.stringify({ modelId: "gpt-test" }),
        method: "POST",
      }),
    );

    await userEvent.click(screen.getByRole("button", { name: "Dismiss model test result" }));
    expect(screen.queryByRole("status")).toBeNull();

    await userEvent.click(testButton);
    const errorToast = await screen.findByRole("alert");
    expect(errorToast).toHaveTextContent(
      "Model test failed for GPT Test: Image output models cannot be tested with a text request",
    );
    await waitFor(() => expect(testButton).toBeEnabled());
  });

  it("shows a model test error toast when the request fails", async () => {
    appTestState.modelTestResponses.push(jsonResponse({ error: "Provider request failed" }, { status: 502 }));
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Test model GPT Test" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Model test failed for GPT Test: Provider request failed",
    );
  });

  it("checks and installs updates from the About settings page", async () => {
    const fetchMock = vi.mocked(fetch);
    window.history.replaceState(null, "", "/settings/about");
    renderApp();

    expect(await screen.findByText("About Foco")).toBeInTheDocument();
    expect(window.location.pathname).toBe("/settings/about");

    await userEvent.click(screen.getByRole("button", { name: "Check for updates" }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Update available")).toBeInTheDocument();
    expect(within(dialog).getByText("Version 0.2.0 is available")).toBeInTheDocument();

    await userEvent.click(within(dialog).getByRole("button", { name: "Install update" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/update/install",
        expect.objectContaining({ method: "POST" }),
      );
    });
    expect(await screen.findByText("Update is installing")).toBeInTheDocument();
  });

  it("waits for update restart health recovery before reloading", async () => {
    appTestState.updateHealthStatuses = [200, 503, 200];
    const reload = vi.fn();
    const waitForReload = installUpdateAndWaitForRestart({ pollMs: 1, reload });

    await expect(waitForReload).resolves.toEqual(appTestState.settingsResponse.update);

    await waitFor(() => expect(reload).toHaveBeenCalledOnce());
    expect(
      vi.mocked(fetch).mock.calls.filter(([url]) => url === "/api/health"),
    ).toHaveLength(3);
  });

  it("saves the automatic update check setting", async () => {
    const fetchMock = vi.mocked(fetch);
    window.history.replaceState(null, "", "/settings/about");
    renderApp();

    const checkbox = await screen.findByRole("checkbox", {
      name: "Automatically check for updates",
    });
    await userEvent.click(checkbox);

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/update/settings",
        expect.objectContaining({
          body: JSON.stringify({ autoCheckEnabled: true }),
          method: "POST",
        }),
      );
    });
  });

  it("orders chat title generation model choices before enabled models", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const trigger = await screen.findByRole("button", { name: /Chat title generation model/ });
    await userEvent.click(trigger);
    const options = (await screen.findAllByRole("option")).map((option) => option.textContent);

    expect(options).toEqual(["Disabled", "Current chat model", "GPT Test"]);
  });

  it("saves plan mode model from plan settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Plan settings" }));

    const trigger = await screen.findByRole("button", { name: /Plan mode model/ });
    await userEvent.click(trigger);
    expect((await screen.findAllByRole("option")).map((option) => option.textContent)).toEqual([
      "Default agent model",
      "GPT Test",
    ]);
    await userEvent.click(screen.getByRole("option", { name: "GPT Test" }));
    await userEvent.click(screen.getByRole("button", { name: "Save plan settings" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(([url]) => url === "/api/settings/plan");
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall![1]?.body))).toEqual({
        mergeAutomationMode: "isolated_auto_once",
        modeModelId: "gpt-test",
      });
    });
    expect(trigger).toHaveAccessibleName("GPT Test Plan mode model");
  });

  it("toggles a skill location with a location-only request", async () => {
    const fetchMock = vi.mocked(fetch);
    const locationPath = "C:\\Users\\fonla\\.agents\\skills";
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));

    const locationToggle = screen.getByRole("checkbox", {
      name: `Enable skill location ${locationPath}`,
    });
    const locationRow = screen.getByText(locationPath).closest("div");
    expect(locationToggle).toBeChecked();
    expect(locationRow).not.toBeNull();
    expect(within(locationRow as HTMLElement).getByRole("checkbox")).toBe(locationToggle);

    await userEvent.click(locationToggle);

    await waitFor(() => expect(locationToggle).not.toBeChecked());
    expect(screen.queryByText("Project memory.")).toBeNull();
    const request = fetchMock.mock.calls.find(([url]) => url === "/api/skills/manual");
    expect(request).toBeDefined();
    const body = JSON.parse(String(request?.[1]?.body));
    expect(body).toEqual({ disabledLocationIds: ["global:agents"] });
    expect(body).not.toHaveProperty("disabled");
    expect(body).not.toHaveProperty("enabled");
    expect(body).not.toHaveProperty("translationModelId");
  });

  it("saves the skill translation model without changing enabled skills", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));

    await userEvent.selectOptions(screen.getByLabelText("Skill translation model"), "gpt-test");

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/skills/manual",
        expect.objectContaining({
          body: JSON.stringify({
            disabled: [],
            enabled: ["global:gitmemo"],
            translationModelId: "gpt-test",
          }),
          method: "POST",
        }),
      );
    });
  });

  it("confirms before deleting a detected skill", async () => {
    const confirmSpy = vi
      .spyOn(window, "confirm")
      .mockReturnValueOnce(false)
      .mockReturnValueOnce(true);
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));

    const deleteButton = await screen.findByRole("button", { name: "Delete skill gitmemo" });
    await userEvent.click(deleteButton);

    expect(confirmSpy).toHaveBeenCalledWith("Delete skill confirmation");
    expect(fetchMock.mock.calls.some(([url]) => url === "/api/skills/delete")).toBe(
      false,
    );

    await userEvent.click(deleteButton);

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/skills/delete",
        expect.objectContaining({
          body: JSON.stringify({ id: "global:gitmemo" }),
          method: "POST",
        }),
      );
    });
    await waitFor(() => {
      expect(screen.queryByText("Global skill")).not.toBeInTheDocument();
    });
    confirmSpy.mockRestore();
  });

  it("shows update actions for store-installed skills", async () => {
    const fetchMock = vi.mocked(fetch);
    const localSkill: ConfiguredSkillSummary = {
      ...appTestState.settingsResponse.skills.detected[0]!,
      description: "Local only.",
      id: "local-only",
      key: "global:local-only",
      name: "local-only",
      path: "C:\\Users\\fonla\\.agents\\skills\\local-only\\SKILL.md",
    };
    const storeSkill: ConfiguredSkillSummary = {
      ...appTestState.settingsResponse.skills.detected[0]!,
      store: {
        skillId: "gitmemo",
        source: "owner/repo",
        updateable: true,
      },
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      skills: {
        ...appTestState.settingsResponse.skills,
        detected: [storeSkill, localSkill],
      },
    };
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Skills" }));

    expect(screen.getByRole("button", { name: "Update all store skills" })).toBeInTheDocument();
    expect(screen.getByText("Store-installed skill")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Update skill gitmemo" })).toHaveAccessibleName(
      "Update skill gitmemo",
    );
    expect(screen.queryByRole("button", { name: "Update skill local-only" })).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Update skill gitmemo" }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/skill-store/update",
        expect.objectContaining({
          body: JSON.stringify({ key: "global:gitmemo" }),
          method: "POST",
        }),
      );
    });

    await userEvent.click(screen.getByRole("button", { name: "Update all store skills" }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/skill-store/update-all",
        expect.objectContaining({ method: "POST" }),
      );
    });
  });

  it("filters plan history by selected workspace", async () => {
    appTestState.settingsResponse = {
      ...settings,
      workspaces: [
        settings.workspaces[0],
        {
          commonCommands: secondaryWorkspace.commonCommands,
          id: secondaryWorkspace.id,
          isDefault: false,
          logoUrl: secondaryWorkspace.logoUrl,
          name: secondaryWorkspace.name,
          path: secondaryWorkspace.path,
          pinned: secondaryWorkspace.pinned,
          terminalShell: secondaryWorkspace.terminalShell,
        },
      ],
    };
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Plan settings" }));

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(([input]) =>
          String(input).startsWith("/api/workspaces/workspace-1/plans?"),
        ),
      ).toBe(true),
    );

    const planHistorySection = (await screen.findByRole("heading", {
      name: "Plan history",
    })).closest("section");
    expect(planHistorySection).not.toBeNull();
    const pageSizeControl = within(planHistorySection as HTMLElement).getByLabelText("Page size");
    const pagination = within(planHistorySection as HTMLElement).getByRole("navigation", {
      name: "Plan history pagination",
    });
    expect(
      pageSizeControl.compareDocumentPosition(pagination) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);

    await userEvent.selectOptions(
      within(planHistorySection as HTMLElement).getByLabelText("Workspace"),
      secondaryWorkspace.id,
    );

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(([input]) =>
          String(input).startsWith("/api/workspaces/workspace-2/plans?"),
        ),
      ).toBe(true),
    );
    expect(
      within(planHistorySection as HTMLElement).getByText("Side project", {
        selector: "p",
      }),
    ).toBeInTheDocument();
  });

  it("shows Spec job history and retries failed jobs", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section");
    expect(specHistorySection).not.toBeNull();
    expect(await within(specHistorySection as HTMLElement).findByText("Side project")).toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).getByText("Default")).toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).getByText("Failed")).toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).getByText("Running")).toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).queryByText("Completed")).not.toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).queryByText("already retried timeout")).not.toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).getByText("model timed out")).toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).getByText("Showing 1-2 of 2")).toBeInTheDocument();
    const retryableOnlyControl = within(specHistorySection as HTMLElement).getByLabelText(
      "Only retryable Spec jobs",
    );
    expect(retryableOnlyControl).toBeChecked();
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(([input]) => {
          const value = String(input);
          return (
            value.startsWith("/api/settings/spec/jobs?") &&
            value.includes("retryableOnly=true")
          );
        }),
      ).toBe(true),
    );
    const pageSizeControl = within(specHistorySection as HTMLElement).getByLabelText("Page size");
    const pagination = within(specHistorySection as HTMLElement).getByRole("navigation", {
      name: "Spec job history pagination",
    });
    expect(
      pageSizeControl.compareDocumentPosition(pagination) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);

    const specTable = within(specHistorySection as HTMLElement).getByRole("table");
    const specTableScroller = specTable.parentElement;
    const settingsScroller = specTable.closest(".settings-content-scroll") as HTMLElement | null;
    expect(specTableScroller).toHaveClass("overflow-x-auto");
    expect(settingsScroller).not.toBeNull();
    if (!specTableScroller || !settingsScroller) {
      throw new Error("Expected Spec job history to live inside settings content scroller");
    }
    settingsScroller.style.overflowY = "auto";
    Object.defineProperties(settingsScroller, {
      clientHeight: { configurable: true, value: 360 },
      scrollHeight: { configurable: true, value: 960 },
    });
    settingsScroller.scrollTop = 0;
    fireEvent.wheel(specTableScroller, { deltaX: 0, deltaY: 120 });
    expect(settingsScroller.scrollTop).toBe(120);

    await userEvent.click(retryableOnlyControl);
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(([input]) => {
          const value = String(input);
          return (
            value.startsWith("/api/settings/spec/jobs?") &&
            value.includes("page=1") &&
            value.includes("retryableOnly=false")
          );
        }),
      ).toBe(true),
    );
    await waitFor(() =>
      expect(within(specHistorySection as HTMLElement).getByText("Completed")).toBeInTheDocument(),
    );
    expect(within(specHistorySection as HTMLElement).getByText("revision 4 / 512 bytes")).toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).getByText("Showing 1-4 of 4")).toBeInTheDocument();

    const failedJob = appTestState.settingsSpecJobsResponse[0];
    appTestState.settingsSpecJobsResponse = [
      {
        ...failedJob,
        job: {
          ...failedJob.job,
          errorMessage: "stale_revision",
          id: "workspace-spec-job-skipped-stale",
          status: "skipped",
        },
      },
      {
        ...failedJob,
        job: {
          ...failedJob.job,
          errorMessage: "workspace_spec_disabled",
          id: "workspace-spec-job-skipped-disabled",
          status: "skipped",
        },
      },
      {
        ...failedJob,
        job: {
          ...failedJob.job,
          errorMessage: "no_update_needed",
          id: "workspace-spec-job-skipped-no-update",
          status: "skipped",
        },
      },
      {
        ...failedJob,
        job: {
          ...failedJob.job,
          errorMessage: "custom_reason",
          id: "workspace-spec-job-skipped-custom",
          status: "skipped",
        },
      },
      failedJob,
    ];
    await userEvent.click(within(specHistorySection as HTMLElement).getByLabelText("Refresh Spec job history"));
    await waitFor(() =>
      expect(
        within(specHistorySection as HTMLElement).getByText(
          "Spec changed before this job could write",
        ),
      ).toBeInTheDocument(),
    );
    expect(within(specHistorySection as HTMLElement).getByText("Workspace Spec is disabled")).toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).getByText("No update needed")).toBeInTheDocument();
    expect(within(specHistorySection as HTMLElement).getByText("custom_reason")).toBeInTheDocument();

    await userEvent.click(
      within(specHistorySection as HTMLElement).getByRole("button", {
        name: "Retry Spec job",
      }),
    );

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-2/spec/jobs/workspace-spec-job-failed/retry",
        expect.objectContaining({ method: "POST" }),
      ),
    );
    await waitFor(() =>
      expect(
        fetchMock.mock.calls.filter(([input]) => {
          const value = String(input);
          return (
            value.startsWith("/api/settings/spec/jobs?") &&
            value.includes("page=1") &&
            value.includes("pageSize=20")
          );
        }).length,
      ).toBeGreaterThanOrEqual(2),
    );
  });

  it("polls Spec job history until a retried job completes", async () => {
    appTestState.settingsSpecJobsResponse = [appTestState.settingsSpecJobsResponse[0]];
    const fetchMock = vi.mocked(fetch);
    let specPoll: (() => void) | null = null;
    const originalSetInterval = window.setInterval.bind(window);
    const pollIntervalId = 7400 as unknown as ReturnType<typeof window.setInterval>;
    const clearIntervalSpy = vi.spyOn(window, "clearInterval");
    vi.spyOn(window, "setInterval").mockImplementation(((
      ...args: Parameters<typeof window.setInterval>
    ) => {
      const [handler, timeout] = args;
      if (timeout === 3000 && typeof handler === "function") {
        specPoll = () => handler();
        return pollIntervalId;
      }
      return (originalSetInterval as (...intervalArgs: unknown[]) => unknown)(
        ...args,
      ) as ReturnType<typeof window.setInterval>;
    }) as typeof window.setInterval);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section") as HTMLElement;
    await userEvent.click(
      within(specHistorySection).getByRole("button", { name: "Retry Spec job" }),
    );

    expect(await within(specHistorySection).findByText("Queued")).toBeInTheDocument();
    await waitFor(() => expect(specPoll).not.toBeNull());
    const activeJob = appTestState.settingsSpecJobsResponse.find(
      (item) => item.job.status === "queued" || item.job.status === "running",
    );
    expect(activeJob).toBeDefined();
    appTestState.settingsSpecJobsResponse = activeJob
      ? appTestState.settingsSpecJobsResponse.map((item) =>
        item.job.id === activeJob.job.id
          ? {
            ...item,
            job: {
              ...item.job,
              completedAt: "2026-06-11T03:13:00Z",
              output: { contentBytes: 640, revision: 5 },
              startedAt: "2026-06-11T03:12:10Z",
              status: "completed",
            },
          }
          : item,
      )
      : appTestState.settingsSpecJobsResponse;

    const poll = specPoll as (() => void) | null;
    if (!poll) {
      throw new Error("Expected Spec job history polling interval to be registered");
    }
    await act(async () => {
      poll();
    });

    await waitFor(() => {
      expect(within(specHistorySection).queryByText("Queued")).not.toBeInTheDocument();
      expect(within(specHistorySection).getAllByText("No Spec jobs")).not.toHaveLength(0);
      expect(clearIntervalSpy).toHaveBeenCalledWith(pollIntervalId);
    });
    expect(
      fetchMock.mock.calls.some(([input]) => {
        const value = String(input);
        return (
          value.startsWith("/api/settings/spec/jobs?") &&
          value.includes("page=1") &&
          value.includes("pageSize=20") &&
          value.includes("retryableOnly=true")
        );
      }),
    ).toBe(true);
  });

  it("hides Spec retry buttons for failed jobs that already have retry jobs", async () => {
    const failedJob = appTestState.settingsSpecJobsResponse[0];
    appTestState.settingsSpecJobsResponse = [
      {
        ...failedJob,
        job: {
          ...failedJob.job,
          hasRetry: true,
          id: "workspace-spec-job-already-retried",
        },
      },
      {
        ...failedJob,
        job: {
          ...failedJob.job,
          hasRetry: false,
          id: "workspace-spec-job-still-retryable",
        },
        workspaceId: "workspace-1",
        workspaceName: "Default",
      },
    ];
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section") as HTMLElement;
    const retryButtons = await within(specHistorySection).findAllByRole("button", {
      name: "Retry Spec job",
    });
    expect(retryButtons).toHaveLength(1);

    await userEvent.click(retryButtons[0]);
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/spec/jobs/workspace-spec-job-still-retryable/retry",
        expect.objectContaining({ method: "POST" }),
      ),
    );
  });

  it("keeps Spec job history retry buttons scoped per failed job", async () => {
    const failedJob = appTestState.settingsSpecJobsResponse[0];
    appTestState.settingsSpecJobsResponse = [
      failedJob,
      {
        ...failedJob,
        job: {
          ...failedJob.job,
          id: "workspace-spec-job-failed-2",
        },
        workspaceId: "workspace-1",
        workspaceName: "Default",
      },
    ];
    const retryGate = deferred<Response>();
    const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];
      if (path === "/api/workspaces/workspace-2/spec/jobs/workspace-spec-job-failed/retry") {
        return retryGate.promise;
      }
      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section") as HTMLElement;
    const retryButtons = await within(specHistorySection).findAllByRole("button", {
      name: "Retry Spec job",
    });

    fireEvent.click(retryButtons[0]);
    fireEvent.click(retryButtons[0]);

    await waitFor(() => expect(retryButtons[0]).toBeDisabled());
    expect(retryButtons[1]).not.toBeDisabled();
    expect(
      fetchMock.mock.calls.filter(
        ([input]) =>
          String(input) ===
          "/api/workspaces/workspace-2/spec/jobs/workspace-spec-job-failed/retry",
      ),
    ).toHaveLength(1);

    retryGate.resolve(jsonResponse({ job: failedJob.job }));
    await waitFor(() => expect(retryButtons[0]).not.toBeDisabled());
  });

  it("keeps concurrent Spec job row operations busy independently", async () => {
    const failedJob = appTestState.settingsSpecJobsResponse[0];
    appTestState.settingsSpecJobsResponse = [
      failedJob,
      {
        ...failedJob,
        job: {
          ...failedJob.job,
          id: "workspace-spec-job-failed-2",
        },
        workspaceId: "workspace-1",
        workspaceName: "Default",
      },
    ];
    const firstRetryGate = deferred<Response>();
    const secondRetryGate = deferred<Response>();
    const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];
      if (path === "/api/workspaces/workspace-2/spec/jobs/workspace-spec-job-failed/retry") {
        return firstRetryGate.promise;
      }
      if (path === "/api/workspaces/workspace-1/spec/jobs/workspace-spec-job-failed-2/retry") {
        return secondRetryGate.promise;
      }
      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section") as HTMLElement;
    const retryButtons = await within(specHistorySection).findAllByRole("button", {
      name: "Retry Spec job",
    });

    fireEvent.click(retryButtons[0]);
    await waitFor(() => expect(retryButtons[0]).toBeDisabled());
    expect(retryButtons[1]).not.toBeDisabled();

    fireEvent.click(retryButtons[1]);
    await waitFor(() => expect(retryButtons[1]).toBeDisabled());
    expect(retryButtons[0]).toBeDisabled();
    expect(retryButtons[0].querySelector(".animate-spin")).not.toBeNull();
    expect(retryButtons[1].querySelector(".animate-spin")).not.toBeNull();

    firstRetryGate.resolve(jsonResponse({ job: failedJob.job }));
    await waitFor(() => expect(retryButtons[0]).not.toBeDisabled());
    expect(retryButtons[1]).toBeDisabled();
    expect(retryButtons[1].querySelector(".animate-spin")).not.toBeNull();

    secondRetryGate.resolve(
      jsonResponse({
        job: {
          ...failedJob.job,
          id: "workspace-spec-job-failed-2",
        },
      }),
    );
    await waitFor(() => expect(retryButtons[1]).not.toBeDisabled());
  });

  it("shows chat titles and deletes failed Spec jobs after confirmation", async () => {
    const longTitle =
      "Very long Spec chat title about architecture contracts and durable product boundaries";
    const failedJob = appTestState.settingsSpecJobsResponse[0];
    appTestState.settingsSpecJobsResponse = [
      {
        ...failedJob,
        chatTitle: longTitle,
      },
      appTestState.settingsSpecJobsResponse[1],
      appTestState.settingsSpecJobsResponse[2],
      appTestState.settingsSpecJobsResponse[3],
    ];
    const fetchMock = vi.mocked(fetch);
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section") as HTMLElement;
    expect(within(specHistorySection).getByText("Chat title")).toBeInTheDocument();
    const longTitleCell = within(specHistorySection).getByText(longTitle);
    expect(longTitleCell).toBeInTheDocument();
    expect(longTitleCell).toHaveAttribute("title", longTitle);

    await userEvent.click(within(specHistorySection).getByLabelText("Only retryable Spec jobs"));
    await waitFor(() =>
      expect(within(specHistorySection).getByText("Already retried chat")).toBeInTheDocument(),
    );
    expect(within(specHistorySection).getAllByText("None").length).toBeGreaterThanOrEqual(1);
    expect(within(specHistorySection).getByText("Completed")).toBeInTheDocument();
    expect(within(specHistorySection).getByText("Running")).toBeInTheDocument();

    const deleteButtons = within(specHistorySection).getAllByRole("button", {
      name: "Delete Spec job",
    });
    // failed + failed-with-retry; not completed/running
    expect(deleteButtons).toHaveLength(2);
    expect(within(specHistorySection).getAllByRole("button", { name: "Retry Spec job" })).toHaveLength(
      1,
    );

    await userEvent.click(deleteButtons[0]);
    expect(confirmSpy).toHaveBeenCalled();
    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-2/spec/jobs/workspace-spec-job-failed",
        expect.objectContaining({ method: "DELETE" }),
      ),
    );
    await waitFor(() =>
      expect(within(specHistorySection).queryByText(longTitle)).not.toBeInTheDocument(),
    );
    expect(
      appTestState.settingsSpecJobsResponse.some(
        (item) => item.job.id === "workspace-spec-job-failed",
      ),
    ).toBe(false);
  });

  it("does not delete a Spec job when confirmation is cancelled", async () => {
    const fetchMock = vi.mocked(fetch);
    vi.spyOn(window, "confirm").mockReturnValue(false);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section") as HTMLElement;
    await userEvent.click(
      within(specHistorySection).getByRole("button", { name: "Delete Spec job" }),
    );

    expect(fetchMock).not.toHaveBeenCalledWith(
      expect.stringMatching(/\/api\/workspaces\/[^/]+\/spec\/jobs\/[^/]+$/),
      expect.objectContaining({ method: "DELETE" }),
    );
    expect(within(specHistorySection).getByText("Side chat about Spec")).toBeInTheDocument();
  });

  it("keeps a failed Spec job when delete request fails", async () => {
    const failedJob = appTestState.settingsSpecJobsResponse[0];
    appTestState.settingsSpecJobsResponse = [failedJob];
    const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.startsWith("http://127.0.0.1")
        ? new URL(url).pathname
        : url.split("?")[0];
      if (
        path === "/api/workspaces/workspace-2/spec/jobs/workspace-spec-job-failed" &&
        (init?.method ?? "GET").toUpperCase() === "DELETE"
      ) {
        return Promise.resolve(
          jsonResponse({ error: "only failed Spec jobs can be deleted" }, { status: 400 }),
        );
      }
      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    vi.spyOn(window, "confirm").mockReturnValue(true);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section") as HTMLElement;
    await userEvent.click(
      within(specHistorySection).getByRole("button", { name: "Delete Spec job" }),
    );

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-2/spec/jobs/workspace-spec-job-failed",
        expect.objectContaining({ method: "DELETE" }),
      ),
    );
    await waitFor(() =>
      expect(screen.getByText("only failed Spec jobs can be deleted")).toBeInTheDocument(),
    );
    expect(within(specHistorySection).getByText("Side chat about Spec")).toBeInTheDocument();
    expect(
      appTestState.settingsSpecJobsResponse.some(
        (item) => item.job.id === "workspace-spec-job-failed",
      ),
    ).toBe(true);
  });

  it("paginates Spec job history", async () => {
    const baseJob = appTestState.settingsSpecJobsResponse[0];
    appTestState.settingsSpecJobsResponse = Array.from({ length: 25 }, (_, index) => ({
      ...baseJob,
      job: {
        ...baseJob.job,
        id: `workspace-spec-job-${index + 1}`,
        createdAt: `2026-06-11T03:${String(59 - index).padStart(2, "0")}:00Z`,
      },
    }));
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section") as HTMLElement;
    const pageSizeControl = await within(specHistorySection).findByLabelText("Page size");
    changeInput(pageSizeControl, "10");

    await waitFor(() => {
      const pageSizeCall = fetchMock.mock.calls.find(([input]) => {
        const value = String(input);
        return (
          value.startsWith("/api/settings/spec/jobs?") &&
          value.includes("page=1") &&
          value.includes("pageSize=10")
        );
      });
      expect(pageSizeCall).toBeDefined();
    });

    await userEvent.click(
      within(specHistorySection).getByRole("button", { name: "Next page" }),
    );

    await waitFor(() => {
      const nextPageCall = fetchMock.mock.calls.find(([input]) => {
        const value = String(input);
        return (
          value.startsWith("/api/settings/spec/jobs?") &&
          value.includes("page=2") &&
          value.includes("pageSize=10")
        );
      });
      expect(nextPageCall).toBeDefined();
    });
  });

  it("corrects Spec job history page after deleting the last item on the last page", async () => {
    const baseJob = appTestState.settingsSpecJobsResponse[0];
    appTestState.settingsSpecJobsResponse = Array.from({ length: 21 }, (_, index) => ({
      ...baseJob,
      chatTitle: `Chat ${index + 1}`,
      job: {
        ...baseJob.job,
        chatId: `chat-${index + 1}`,
        id: `workspace-spec-job-${index + 1}`,
        createdAt: `2026-06-11T04:${String(59 - index).padStart(2, "0")}:00Z`,
      },
    }));
    const fetchMock = vi.mocked(fetch);
    vi.spyOn(window, "confirm").mockReturnValue(true);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const specHistorySection = (await screen.findByRole("heading", {
      name: "Spec job history",
    })).closest("section") as HTMLElement;
    const pageSizeControl = await within(specHistorySection).findByLabelText("Page size");
    changeInput(pageSizeControl, "20");

    await waitFor(() =>
      expect(within(specHistorySection).getByText("Showing 1-20 of 21")).toBeInTheDocument(),
    );
    await userEvent.click(
      within(specHistorySection).getByRole("button", { name: "Next page" }),
    );
    await waitFor(() =>
      expect(within(specHistorySection).getByText("Chat 21")).toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(within(specHistorySection).getByText("Showing 21-21 of 21")).toBeInTheDocument(),
    );

    const pageOneListRequestCount = () =>
      fetchMock.mock.calls.filter(([input]) => {
        const value = String(input);
        return (
          value.startsWith("/api/settings/spec/jobs?") &&
          value.includes("page=1") &&
          value.includes("pageSize=20")
        );
      }).length;
    const listRequestsBeforeDelete = pageOneListRequestCount();

    await userEvent.click(
      within(specHistorySection).getByRole("button", { name: "Delete Spec job" }),
    );

    await waitFor(() =>
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-2/spec/jobs/workspace-spec-job-21",
        expect.objectContaining({ method: "DELETE" }),
      ),
    );
    await waitFor(() => {
      expect(pageOneListRequestCount()).toBeGreaterThan(listRequestsBeforeDelete);
      expect(within(specHistorySection).queryByText("Chat 21")).not.toBeInTheDocument();
      expect(within(specHistorySection).getByText("Showing 1-20 of 20")).toBeInTheDocument();
    });
  });

  it("localizes the Spec settings surface", async () => {
    const zhSettings = {
      ...settings,
      general: { ...settings.general, language: "zh-CN" },
    };
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        return path === "/api/settings"
          ? jsonResponse(zhSettings)
          : mockFetch(input, init);
      }),
    );

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "设置" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "设置" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    expect(await screen.findByRole("heading", { name: "Spec 设置" })).toBeInTheDocument();
    expect(screen.getByText("自动 Spec")).toBeInTheDocument();
    expect(await screen.findByText("Spec 任务历史")).toBeInTheDocument();
    expect(screen.getByText("每页数量")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Spec 任务历史分页" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "刷新 Spec 任务历史" })).toBeInTheDocument();
    expect(screen.getByLabelText("仅显示可重试记录")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "重试 Spec 任务" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除 Spec 任务" })).toBeInTheDocument();
    expect(screen.getByText("会话标题")).toBeInTheDocument();
    expect(screen.getByText("自动化")).toBeInTheDocument();
    expect(screen.getByText("成功聊天轮次结束后更新已启用的工作区 Spec。")).toBeInTheDocument();
    const automation = screen.getByText("自动化").closest("fieldset");
    expect(automation).not.toBeNull();
    expect(within(automation as HTMLElement).getByLabelText("Spec 生成模型")).toBeInTheDocument();
    expect(within(automation as HTMLElement).getByLabelText("启用自动 Spec")).toBeInTheDocument();
    expect(within(automation as HTMLElement).getByLabelText("Spec LLM 超时 ms")).toBeInTheDocument();
    expect(screen.queryByLabelText("Spec 生成系统提示词")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Spec 更新系统提示词")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Spec 生成提示词")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Spec 更新提示词")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "保存 Spec 设置" })).not.toBeInTheDocument();

    await userEvent.click(within(settingsNav).getByRole("button", { name: "关于" }));
    expect(await screen.findByRole("heading", { name: "关于 Foco" })).toBeInTheDocument();
  });

  it("localizes provider model redirect controls", async () => {
    const zhSettings = {
      ...settings,
      general: { ...settings.general, language: "zh-CN" },
      providers: [
        {
          ...settings.providers[0],
          modelRedirects: [
            { from: "qwen/qwen3.6-35b-a3b", to: "qwen3.6-35b-a3b" },
          ],
        },
        settings.providers[1],
      ],
    };
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        return path === "/api/settings"
          ? jsonResponse(zhSettings)
          : mockFetch(input, init);
      }),
    );

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "设置" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "设置" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "供应商" }));

    expect(await screen.findByText("qwen/qwen3.6-35b-a3b -> qwen3.6-35b-a3b")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "编辑供应商 OpenAI" }));
    expect(screen.getByRole("heading", { name: "模型名称重定向" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "添加重定向" })).toBeInTheDocument();
    expect(screen.getByLabelText("上游模型")).toHaveValue("qwen/qwen3.6-35b-a3b");
    expect(screen.getByLabelText("本地模型")).toHaveValue("qwen3.6-35b-a3b");
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

  it("prefills provider protocol and base URL from the service menu", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    await userEvent.click(screen.getByRole("button", { name: "Add provider" }));

    await expectSelectedOption(screen.getByLabelText("Protocol"), "openai-responses");
    expect(screen.getByLabelText("Base URL")).toHaveValue("https://api.openai.com/v1");

    await userEvent.click(screen.getByRole("button", { name: /^DeepSeek/ }));

    expect(screen.getByLabelText("Name")).toHaveValue("DeepSeek");
    await expectSelectedOption(screen.getByLabelText("Protocol"), "deepseek");
    expect(screen.getByLabelText("Base URL")).toHaveValue("https://api.deepseek.com/v1");
  });

  it("uses full-width rounded text fields in the provider configuration dialog", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    await userEvent.click(screen.getByRole("button", { name: "Edit provider OpenAI" }));

    const providerDialog = screen.getByRole("form", { name: "Provider configuration" });
    const nameInput = within(providerDialog).getByLabelText("Name");
    const baseUrlInput = within(providerDialog).getByLabelText("Base URL");

    for (const input of [nameInput, baseUrlInput]) {
      expect(input).toHaveClass("rounded-lg");
      expect(input.closest(".textfield")).toHaveClass("textfield--full-width");
    }
  });

  it("keeps a single base URL and disables proxy for OpenAI Responses WebSocket", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    await userEvent.click(screen.getByRole("button", { name: "Add provider" }));

    await userEvent.click(screen.getByRole("checkbox", { name: "Enable AI API proxy" }));
    expect(screen.getByRole("checkbox", { name: "Enable AI API proxy" })).toBeChecked();

    await userEvent.selectOptions(
      screen.getByLabelText("Protocol"),
      "openai-responses-websocket",
    );

    await expectSelectedOption(
      screen.getByLabelText("Protocol"),
      "openai-responses-websocket",
    );
    expect(screen.getByLabelText("Base URL")).toHaveValue("https://api.openai.com/v1");
    expect(screen.queryByLabelText("WebSocket URL")).not.toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Enable AI API proxy" })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Enable AI API proxy" })).toBeDisabled();
    expect(
      screen.getByText(
        "AI API proxy is not supported for the OpenAI Responses WebSocket protocol in this release.",
      ),
    ).toBeInTheDocument();
  });

  it("keeps custom provider base URL when changing protocol", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    await userEvent.click(screen.getByRole("button", { name: "Add provider" }));

    const baseUrlInput = screen.getByLabelText("Base URL");
    await userEvent.clear(baseUrlInput);
    await userEvent.type(baseUrlInput, "https://proxy.example.test/v1");
    await userEvent.selectOptions(screen.getByLabelText("Protocol"), "openai-chat");

    await expectSelectedOption(screen.getByLabelText("Protocol"), "openai-chat");
    expect(baseUrlInput).toHaveValue("https://proxy.example.test/v1");
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
      expect(document.documentElement.dataset.theme).toBe("dark");
      expect(document.documentElement.classList.contains("dark")).toBe(true);
    });
  });

  it("keeps the theme button interactive while the optimistic save is pending", async () => {
    const fetchMock = vi.mocked(fetch);
    const firstThemeSave = deferred<Response>();
    const themeSaveBodies: string[] = [];
    let delayNextThemeSave = true;
    fetchMock.mockImplementation((input, init) => {
      const path = String(input).split("?")[0];
      if (path === "/api/settings/general") {
        themeSaveBodies.push(String(init?.body ?? ""));
      }
      if (path === "/api/settings/general" && delayNextThemeSave) {
        delayNextThemeSave = false;
        return firstThemeSave.promise;
      }
      return mockFetch(input, init);
    });
    renderApp();

    await userEvent.click(
      await screen.findByRole("button", { name: "Switch to dark theme" }),
    );

    const lightThemeButton = await screen.findByRole("button", {
      name: "Switch to light theme",
    });
    expect(lightThemeButton).not.toBeDisabled();
    await userEvent.click(lightThemeButton);

    await waitFor(() => {
      expect(document.documentElement.dataset.theme).toBe("light");
    });
    expect(themeSaveBodies).toHaveLength(1);
    expect(themeSaveBodies[0]).toContain('"theme":"dark"');

    await act(async () => {
      firstThemeSave.resolve(
        jsonResponse({
          ...settings,
          general: { ...settings.general, theme: "dark" },
        }),
      );
      await firstThemeSave.promise;
    });

    await waitFor(() => {
      expect(themeSaveBodies).toHaveLength(2);
      expect(themeSaveBodies[1]).toContain('"theme":"light"');
      expect(document.documentElement.dataset.theme).toBe("light");
      expect(
        screen.getByRole("button", { name: "Switch to dark theme" }),
      ).not.toBeDisabled();
    });
  });

  it("saves the app theme from general settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const themeSelect = await screen.findByRole("button", { name: /Theme/ });
    await waitFor(() => {
      expect(themeSelect).not.toBeDisabled();
      expect(themeSelect).not.toBeDisabled();
    });
    await userEvent.selectOptions(themeSelect, "dark");

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
      expect(document.documentElement.dataset.theme).toBe("dark");
      expect(document.documentElement.classList.contains("dark")).toBe(true);
    });
  });

  it("saves auto start from general settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(
      await screen.findByRole("checkbox", {
        name: "Start Foco at startup",
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

  it("saves and reloads runtime tool-state compression from general settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const compressionToggle = await screen.findByRole("checkbox", {
      name: "Runtime tool-state compression",
    });

    expect(compressionToggle).not.toBeChecked();
    expect(
      screen.getByText(
        "At 80% context usage, replace older tool messages with compact snapshots. This breaks the provider's context cache.",
      ),
    ).toBeInTheDocument();

    await userEvent.click(compressionToggle);
    await userEvent.click(screen.getByRole("button", { name: "Save general settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/general",
        expect.objectContaining({
          body: expect.stringContaining('"runtimeToolStateCompressionEnabled":true'),
          method: "POST",
        }),
      );
    });

    await userEvent.click(screen.getByRole("button", { name: "Reload general settings" }));
    expect(
      await screen.findByRole("checkbox", {
        name: "Runtime tool-state compression",
      }),
    ).toBeChecked();
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

  it("saves spec settings", async () => {
    const fetchMock = vi.mocked(fetch);
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      spec: {
        ...appTestState.settingsResponse.spec,
        generationSystemPrompt: "Keep this generation prompt",
        updateSystemPrompt: "Keep this update prompt",
      },
    };
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    expect(await screen.findByText("Spec settings")).toBeInTheDocument();
    expect(screen.queryByLabelText("Spec generation system prompt")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Spec update system prompt")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Save spec settings" })).not.toBeInTheDocument();

    const automation = screen.getByText("Automation").closest("fieldset");
    expect(automation).not.toBeNull();
    expect(
      within(automation as HTMLElement).getByLabelText("Spec generation model"),
    ).toBeInTheDocument();

    await userEvent.click(screen.getByLabelText("Enable Auto Spec"));
    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(([url]) => url === "/api/settings/spec");
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toEqual({
        autoEnabled: false,
        generationModelId: null,
        llmTimeoutMs: 300000,
      });
      expect(JSON.parse(String(saveCall?.[1]?.body))).not.toHaveProperty(
        "generationSystemPrompt",
      );
      expect(JSON.parse(String(saveCall?.[1]?.body))).not.toHaveProperty(
        "updateSystemPrompt",
      );
    });

    await userEvent.selectOptions(screen.getByLabelText("Spec generation model"), "gpt-test");
    await waitFor(() => {
      const modelCalls = fetchMock.mock.calls.filter(([url]) => url === "/api/settings/spec");
      const lastBody = JSON.parse(String(modelCalls.at(-1)?.[1]?.body));
      expect(lastBody).toEqual({
        autoEnabled: false,
        generationModelId: "gpt-test",
        llmTimeoutMs: 300000,
      });
    });

    changeInput(screen.getByLabelText("Spec LLM timeout ms"), "90000");
    fireEvent.blur(screen.getByLabelText("Spec LLM timeout ms"));
    await waitFor(() => {
      const timeoutCalls = fetchMock.mock.calls.filter(([url]) => url === "/api/settings/spec");
      const lastBody = JSON.parse(String(timeoutCalls.at(-1)?.[1]?.body));
      expect(lastBody).toEqual({
        autoEnabled: false,
        generationModelId: "gpt-test",
        llmTimeoutMs: 90000,
      });
    });

    // Automation-only saves must not clear Spec prompts stored from the Prompts page.
    expect(appTestState.settingsResponse.spec.generationSystemPrompt).toBe(
      "Keep this generation prompt",
    );
    expect(appTestState.settingsResponse.spec.updateSystemPrompt).toBe(
      "Keep this update prompt",
    );
  });

  it("lists only Spec-eligible generation models in the Spec model dropdown", async () => {
    const disabledModel: ConfiguredModelSummary = {
      ...appTestState.settingsResponse.configuredModels[0]!,
      displayName: "Disabled Model",
      enabled: false,
      id: "disabled-model",
    };
    const providerlessModel: ConfiguredModelSummary = {
      ...appTestState.settingsResponse.configuredModels[0]!,
      activeProviderId: null,
      displayName: "Providerless Model",
      id: "providerless-model",
    };
    const disabledProviderModel: ConfiguredModelSummary = {
      ...appTestState.settingsResponse.configuredModels[0]!,
      activeProviderId: "disabled-provider",
      displayName: "Disabled Provider Model",
      id: "disabled-provider-model",
      providerIds: ["disabled-provider"],
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      configuredModels: [
        ...appTestState.settingsResponse.configuredModels,
        disabledModel,
        providerlessModel,
        disabledProviderModel,
      ],
      providers: [
        ...appTestState.settingsResponse.providers,
        {
          ...appTestState.settingsResponse.providers[0]!,
          enabled: false,
          id: "disabled-provider",
          name: "Disabled Provider",
        },
      ],
    };

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const modelSelect = await screen.findByLabelText("Spec generation model");
    await userEvent.click(modelSelect);
    const optionLabels = (await screen.findAllByRole("option"))
      .map((option) => option.textContent);

    expect(optionLabels).toContain("Automatic");
    expect(optionLabels).toContain("GPT Test");
    expect(optionLabels).not.toContain("Disabled Model");
    expect(optionLabels).not.toContain("Providerless Model");
    expect(optionLabels).not.toContain("Disabled Provider Model");
  });

  it("keeps an unavailable historical Spec generation model selected with an explicit label", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      configuredModels: [
        ...appTestState.settingsResponse.configuredModels,
        {
          ...appTestState.settingsResponse.configuredModels[0]!,
          displayName: "Disabled Model",
          enabled: false,
          id: "disabled-model",
        },
        {
          ...appTestState.settingsResponse.configuredModels[0]!,
          activeProviderId: null,
          displayName: "Providerless Model",
          id: "providerless-model",
        },
      ],
      spec: {
        ...appTestState.settingsResponse.spec,
        generationModelId: "disabled-model",
      },
    };

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const modelSelect = await screen.findByLabelText("Spec generation model");
    await userEvent.click(modelSelect);
    expect(
      screen.getByRole("option", {
        name: "Model unavailable: Disabled Model",
      }),
    ).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("option", { name: "Automatic" })).toBeInTheDocument();
    // Historical disabled stays selected; other ineligible models stay off the option list.
    expect(
      screen.queryByRole("option", { name: "Providerless Model" }),
    ).toBeNull();
    expect(
      screen.queryByRole("option", {
        name: "Model unavailable: Providerless Model",
      }),
    ).toBeNull();
  });

  it("shows a historical providerless Spec generation model as unavailable", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      configuredModels: [
        ...appTestState.settingsResponse.configuredModels,
        {
          ...appTestState.settingsResponse.configuredModels[0]!,
          activeProviderId: null,
          displayName: "Providerless Model",
          id: "providerless-model",
        },
      ],
      spec: {
        ...appTestState.settingsResponse.spec,
        generationModelId: "providerless-model",
      },
    };

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const modelSelect = await screen.findByLabelText("Spec generation model");
    await userEvent.click(modelSelect);
    expect(
      screen.getByRole("option", {
        name: "Model unavailable: Providerless Model",
      }),
    ).toHaveAttribute("aria-selected", "true");
  });

  it("keeps Spec save errors when Spec job history reloads and rolls back failed saves", async () => {
    const fetchMock = vi.mocked(fetch);
    const originalFetch = mockFetch;
    let jobsFetchCount = 0;
    fetchMock.mockImplementation(async (input, init) => {
      const path = String(input);
      if (path === "/api/settings/spec" && init?.method === "POST") {
        return jsonResponse(
          {
            error:
              "spec.generation_model_id references missing, disabled, or providerless model 'broken-model'",
          },
          { status: 400 },
        );
      }
      if (path.startsWith("/api/settings/spec/jobs?")) {
        jobsFetchCount += 1;
      }
      return originalFetch(input, init);
    });

    // Keep a running job so the Spec section registers the 3s poll interval.
    appTestState.settingsSpecJobsResponse = [
      {
        ...appTestState.settingsSpecJobsResponse[1]!,
        job: {
          ...appTestState.settingsSpecJobsResponse[1]!.job,
          status: "running",
        },
      },
    ];
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      configuredModels: [
        ...appTestState.settingsResponse.configuredModels,
        {
          ...appTestState.settingsResponse.configuredModels[0]!,
          displayName: "Broken Model",
          id: "broken-model",
        },
      ],
      spec: {
        ...appTestState.settingsResponse.spec,
        autoEnabled: true,
        generationModelId: null,
        generationSystemPrompt: "Do not clear this generation prompt",
        updateSystemPrompt: "Do not clear this update prompt",
      },
    };

    let specPoll: (() => void) | null = null;
    const originalSetInterval = window.setInterval.bind(window);
    vi.spyOn(window, "setInterval").mockImplementation((
      ...args: Parameters<typeof window.setInterval>
    ) => {
      const [handler, timeout] = args;
      if (timeout === 3000 && typeof handler === "function") {
        specPoll = () => handler();
        return 8400 as unknown as ReturnType<typeof window.setInterval>;
      }
      return (originalSetInterval as (...intervalArgs: unknown[]) => unknown)(
        ...args,
      ) as ReturnType<typeof window.setInterval>;
    });

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const modelSelect = await screen.findByLabelText("Spec generation model");
    expect(modelSelect).toHaveValue("");

    await userEvent.selectOptions(modelSelect, "broken-model");

    const saveError = await screen.findByText(
      /spec\.generation_model_id references missing, disabled, or providerless model 'broken-model'/,
    );
    expect(saveError).toBeInTheDocument();
    // Failed save must not leave a pseudo-persisted selection in the form.
    await waitFor(() => {
      expect(screen.getByLabelText("Spec generation model")).toHaveValue("");
    });
    expect(appTestState.settingsResponse.spec.generationModelId).toBeNull();
    expect(appTestState.settingsResponse.spec.generationSystemPrompt).toBe(
      "Do not clear this generation prompt",
    );
    expect(appTestState.settingsResponse.spec.updateSystemPrompt).toBe(
      "Do not clear this update prompt",
    );

    const jobsBeforePoll = jobsFetchCount;
    await waitFor(() => expect(specPoll).not.toBeNull());
    const poll = specPoll as (() => void) | null;
    if (!poll) {
      throw new Error("Expected Spec job history polling interval to be registered");
    }
    await act(async () => {
      poll();
    });
    await waitFor(() => {
      expect(jobsFetchCount).toBeGreaterThan(jobsBeforePoll);
    });

    // Jobs polling must not clear the automation save error.
    expect(
      screen.getByText(
        /spec\.generation_model_id references missing, disabled, or providerless model 'broken-model'/,
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Spec generation model")).toHaveValue("");

    await userEvent.click(screen.getByRole("button", { name: "Refresh Spec job history" }));
    await waitFor(() => {
      expect(jobsFetchCount).toBeGreaterThan(jobsBeforePoll + 1);
    });
    expect(
      screen.getByText(
        /spec\.generation_model_id references missing, disabled, or providerless model 'broken-model'/,
      ),
    ).toBeInTheDocument();
  });

  it("serializes Spec saves with latest-wins coalescing (A→B→C drops B)", async () => {
    const fetchMock = vi.mocked(fetch);
    const originalFetch = mockFetch;
    const firstSaveGate = deferred<Response>();
    let postCount = 0;
    const postBodies: Array<Record<string, unknown>> = [];
    const postStartOrder: number[] = [];

    fetchMock.mockImplementation(async (input, init) => {
      const path = String(input);
      if (path === "/api/settings/spec" && init?.method === "POST") {
        postCount += 1;
        const startedAt = postCount;
        postStartOrder.push(startedAt);
        const body = JSON.parse(String(init.body ?? "{}")) as Record<string, unknown>;
        postBodies.push(body);
        if (startedAt === 1) {
          await firstSaveGate.promise;
        }
        return originalFetch(input, init);
      }
      return originalFetch(input, init);
    });

    const altModel: ConfiguredModelSummary = {
      ...appTestState.settingsResponse.configuredModels[0]!,
      displayName: "GPT Alt",
      id: "gpt-alt",
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      configuredModels: [...appTestState.settingsResponse.configuredModels, altModel],
      spec: {
        ...appTestState.settingsResponse.spec,
        autoEnabled: true,
        generationModelId: null,
        generationSystemPrompt: "Keep generation prompt across automation saves",
        updateSystemPrompt: "Keep update prompt across automation saves",
      },
    };

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const modelSelect = await screen.findByLabelText("Spec generation model");
    await userEvent.click(modelSelect);
    expect(screen.getByRole("option", { name: "GPT Alt" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");

    // Request A: toggle Auto Spec off while the POST is held open.
    await userEvent.click(screen.getByLabelText("Enable Auto Spec"));
    await waitFor(() => {
      expect(postCount).toBe(1);
    });

    // Pending B then C while A is still in flight — only the latest pending snapshot (C) may ship.
    await userEvent.selectOptions(modelSelect, "gpt-alt");
    await expectSelectedOption(screen.getByLabelText("Spec generation model"), "gpt-alt");
    expect(postCount).toBe(1);

    await userEvent.selectOptions(modelSelect, "gpt-test");
    await expectSelectedOption(screen.getByLabelText("Spec generation model"), "gpt-test");
    expect(postCount).toBe(1);

    firstSaveGate.resolve(undefined as unknown as Response);
    await waitFor(() => {
      expect(postCount).toBe(2);
    });
    await waitFor(() => {
      expect(appTestState.settingsResponse.spec.generationModelId).toBe("gpt-test");
    });
    await expectSelectedOption(screen.getByLabelText("Spec generation model"), "gpt-test");

    // Serial order: only A then C. Intermediate B (gpt-alt) must never be POSTed.
    expect(postStartOrder).toEqual([1, 2]);
    expect(postBodies).toHaveLength(2);
    expect(postBodies[0]).toEqual({
      autoEnabled: false,
      generationModelId: null,
      llmTimeoutMs: 300000,
    });
    expect(postBodies[1]).toEqual({
      autoEnabled: false,
      generationModelId: "gpt-test",
      llmTimeoutMs: 300000,
    });
    expect(postBodies.some((body) => body.generationModelId === "gpt-alt")).toBe(false);
    for (const body of postBodies) {
      expect(body).not.toHaveProperty("generationSystemPrompt");
      expect(body).not.toHaveProperty("updateSystemPrompt");
    }
    expect(appTestState.settingsResponse.spec.generationSystemPrompt).toBe(
      "Keep generation prompt across automation saves",
    );
    expect(appTestState.settingsResponse.spec.updateSystemPrompt).toBe(
      "Keep update prompt across automation saves",
    );
    expect(screen.getByLabelText("Enable Auto Spec")).not.toBeChecked();
  });

  it("clears a visible Spec save error after a later successful save", async () => {
    const fetchMock = vi.mocked(fetch);
    const originalFetch = mockFetch;
    let failNextSpecSave = true;
    const secondSaveGate = deferred<Response>();
    let postCount = 0;

    fetchMock.mockImplementation(async (input, init) => {
      const path = String(input);
      if (path === "/api/settings/spec" && init?.method === "POST") {
        postCount += 1;
        if (failNextSpecSave) {
          failNextSpecSave = false;
          return jsonResponse({ error: "first Spec save failed" }, { status: 400 });
        }
        // Hold the recovery save open so we can assert the prior error stays
        // until success settles (not merely until the next attempt starts).
        await secondSaveGate.promise;
        return originalFetch(input, init);
      }
      return originalFetch(input, init);
    });

    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      configuredModels: [
        ...appTestState.settingsResponse.configuredModels,
        {
          ...appTestState.settingsResponse.configuredModels[0]!,
          displayName: "Broken Model",
          id: "broken-model",
        },
      ],
      spec: {
        ...appTestState.settingsResponse.spec,
        autoEnabled: true,
        generationModelId: null,
        generationSystemPrompt: "Preserve generation prompt after error recovery",
        updateSystemPrompt: "Preserve update prompt after error recovery",
      },
    };

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const modelSelect = await screen.findByLabelText("Spec generation model");
    expect(modelSelect).toHaveValue("");

    // Let a failed save settle with no pending follow-up so the error is actually rendered.
    await userEvent.selectOptions(modelSelect, "broken-model");
    expect(await screen.findByText("first Spec save failed")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByLabelText("Spec generation model")).toHaveValue("");
    });
    expect(appTestState.settingsResponse.spec.generationModelId).toBeNull();
    expect(postCount).toBe(1);

    // Start a recovery save but keep it in flight: the prior error must remain.
    await userEvent.selectOptions(screen.getByLabelText("Spec generation model"), "gpt-test");
    await waitFor(() => {
      expect(postCount).toBe(2);
    });
    expect(screen.getByText("first Spec save failed")).toBeInTheDocument();
    expect(appTestState.settingsResponse.spec.generationModelId).toBeNull();

    // Only after the later save succeeds may the visible save error clear.
    secondSaveGate.resolve(undefined as unknown as Response);
    await waitFor(() => {
      expect(appTestState.settingsResponse.spec.generationModelId).toBe("gpt-test");
    });
    await expectSelectedOption(screen.getByLabelText("Spec generation model"), "gpt-test");
    expect(screen.queryByText("first Spec save failed")).not.toBeInTheDocument();
    expect(appTestState.settingsResponse.spec.generationSystemPrompt).toBe(
      "Preserve generation prompt after error recovery",
    );
    expect(appTestState.settingsResponse.spec.updateSystemPrompt).toBe(
      "Preserve update prompt after error recovery",
    );
  });

  it("does not let a stale settings GET overwrite a newer Spec save", async () => {
    const fetchMock = vi.mocked(fetch);
    const originalFetch = mockFetch;
    let holdNextSettingsGet = false;
    const staleGetGate = deferred<void>();
    let staleGenerationModelId: string | null | undefined;

    fetchMock.mockImplementation(async (input, init) => {
      const path = String(input);
      const method = String(init?.method ?? "GET").toUpperCase();
      if (path === "/api/settings" && method === "GET" && holdNextSettingsGet) {
        holdNextSettingsGet = false;
        const staleSnapshot = {
          ...appTestState.settingsResponse,
          configuredModels: [...appTestState.settingsResponse.configuredModels],
          spec: { ...appTestState.settingsResponse.spec },
        };
        staleGenerationModelId = staleSnapshot.spec.generationModelId;
        await staleGetGate.promise;
        return jsonResponse(staleSnapshot);
      }
      return originalFetch(input, init);
    });

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    expect(await screen.findByText("General settings")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByLabelText("Reload general settings")).not.toBeDisabled();
    });

    holdNextSettingsGet = true;
    await userEvent.click(screen.getByRole("button", { name: "Reload general settings" }));
    await waitFor(() => {
      expect(staleGenerationModelId).toBeDefined();
    });

    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Spec" }));

    const modelSelect = await screen.findByLabelText("Spec generation model");
    await userEvent.click(modelSelect);
    expect(screen.getByRole("option", { name: "GPT Test" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await userEvent.selectOptions(modelSelect, "gpt-test");
    await waitFor(() => {
      expect(appTestState.settingsResponse.spec.generationModelId).toBe("gpt-test");
    });
    await expectSelectedOption(screen.getByLabelText("Spec generation model"), "gpt-test");

    staleGetGate.resolve();
    await act(async () => {
      await Promise.resolve();
    });
    await expectSelectedOption(screen.getByLabelText("Spec generation model"), "gpt-test");
    expect(staleGenerationModelId).not.toBe("gpt-test");
  });

  it("saves memory settings", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Memory settings")).toBeInTheDocument();
    expect((await screen.findAllByText(activeMemory.fact)).length).toBeGreaterThan(0);

    const budgetInput = screen.getByLabelText("Memory context budget %");
    expect(budgetInput).toHaveValue("12");
    expect(
      screen.getByText(
        "Percent of the model's available message tokens that matched memories may occupy. This is a token budget, not a fixed number of memories.",
      ),
    ).toBeInTheDocument();

    const dreamControlOrder = [
      screen.getByLabelText("Enable Dream"),
      screen.getByLabelText("Enable Auto Dream"),
      screen.getByLabelText("Create transcript chat"),
      screen.getByLabelText("Dream mode"),
      screen.getByLabelText("Dream model"),
      screen.getByLabelText("Workspace interval days"),
    ];
    for (const [index, control] of dreamControlOrder.entries()) {
      const nextControl = dreamControlOrder[index + 1];
      if (nextControl) {
        expect(
          control.compareDocumentPosition(nextControl) &
            Node.DOCUMENT_POSITION_FOLLOWING,
        ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      }
    }

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
    changeInput(screen.getByLabelText("Extraction LLM timeout ms"), "130000");
    changeInput(screen.getByLabelText("Retrieval LLM timeout ms"), "70000");
    changeInput(budgetInput, "25");
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
    changeInput(screen.getByLabelText("Dream LLM timeout ms"), "140000");
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
        extractionLlmTimeoutMs: 130000,
        retrievalLlmTimeoutMs: 70000,
        contextBudgetPercent: 25,
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
          llmTimeoutMs: 140000,
        },
      });
    });

    await waitFor(() => {
      expect(screen.getByLabelText("Memory context budget %")).toHaveValue("25");
    });
  });

  it("rejects invalid memory context budget percent without saving", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Memory settings")).toBeInTheDocument();
    const budgetInput = screen.getByLabelText("Memory context budget %");
    expect(budgetInput).toHaveValue("12");

    const saveCallsBefore = fetchMock.mock.calls.filter(
      ([url]) => url === "/api/settings/memory",
    ).length;

    changeInput(budgetInput, "0");
    await userEvent.click(screen.getByRole("button", { name: "Save memory settings" }));

    expect(
      await screen.findByText("Memory context budget % must be a whole number from 1 to 100"),
    ).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(([url]) => url === "/api/settings/memory"),
    ).toHaveLength(saveCallsBefore);

    changeInput(budgetInput, "101");
    await userEvent.click(screen.getByRole("button", { name: "Save memory settings" }));

    expect(
      await screen.findByText("Memory context budget % must be a whole number from 1 to 100"),
    ).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(([url]) => url === "/api/settings/memory"),
    ).toHaveLength(saveCallsBefore);

    changeInput(budgetInput, "12.5");
    await userEvent.click(screen.getByRole("button", { name: "Save memory settings" }));

    expect(
      await screen.findByText("Memory context budget % must be a whole number from 1 to 100"),
    ).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(([url]) => url === "/api/settings/memory"),
    ).toHaveLength(saveCallsBefore);

    changeInput(budgetInput, "");
    await userEvent.click(screen.getByRole("button", { name: "Save memory settings" }));

    expect(
      await screen.findByText("Memory context budget % must be a whole number from 1 to 100"),
    ).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(([url]) => url === "/api/settings/memory"),
    ).toHaveLength(saveCallsBefore);
  });

  it("uses semantic colors for Dream history status pills", async () => {
    const dreamJobs = [
      { ...memoryDreamJob, id: "dream-job-completed", status: "completed" },
      { ...memoryDreamJob, id: "dream-job-failed", status: "failed" },
      { ...memoryDreamJob, id: "dream-job-running", status: "running" },
      { ...memoryDreamJob, id: "dream-job-cancelled", status: "cancelled" },
    ];

    vi.mocked(fetch).mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/memory/dream/jobs") {
        return Promise.resolve(
          jsonResponse({
            jobs: dreamJobs,
            page: 1,
            pageSize: 10,
            totalCount: dreamJobs.length,
            totalPages: 1,
          }),
        );
      }

      return Promise.resolve(mockFetch(input, init));
    });

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    const dreamTable = await screen.findByRole("table");
    const dreamStatusPill = (label: string) => {
      const text = within(dreamTable).getByText(label);
      return text.parentElement as HTMLElement;
    };
    expect(dreamStatusPill("Completed")).toHaveClass(
      "bg-[var(--success-soft)]",
      "text-[var(--success-soft-foreground)]",
    );
    expect(dreamStatusPill("Failed")).toHaveClass(
      "bg-[var(--danger-soft)]",
      "text-[var(--danger)]",
    );
    expect(dreamStatusPill("Running")).toHaveClass(
      "bg-[var(--warning-soft)]",
      "text-[var(--warning)]",
    );
    expect(dreamStatusPill("Cancelled")).toHaveClass(
      "bg-[var(--surface-secondary)]",
      "text-[var(--muted)]",
    );
    expect(screen.queryByText("Some remote Dream history is unavailable")).toBeNull();
  });

  it("keeps Dream history visible when a remote workspace is partially unavailable", async () => {
    const job: MemoryDreamJobSummary = {
      ...memoryDreamJob,
      mode: "llm",
      scope: "workspace",
      status: "completed",
      triggerType: "manual",
    };
    appTestState.memoryDreamJobsResponses.push({
      jobs: [job],
      page: 1,
      pageSize: 10,
      partialUnavailable: [
        {
          workspaceId: workspace.id,
          reason: "notConnected",
          message: "Remote Dream history is unavailable because the workspace is not connected.",
        },
      ],
      totalCount: 1,
      totalPages: 1,
    });
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    const partialAvailability = await screen.findByText(
      "Some remote Dream history is unavailable",
    );
    const partialNotice = partialAvailability.closest('[role="status"]');
    if (!partialNotice) {
      throw new Error("Expected partial Dream availability notice");
    }
    expect(partialNotice).toHaveTextContent(workspace.name);
    expect(partialNotice).toHaveTextContent("Remote workspace is not connected");
    expect(screen.getByRole("table")).toHaveTextContent("Completed");
    expect(screen.queryByText("invalid memory data: workspace path is not a directory:")).toBeNull();
  });

  it("uses the existing Dream history error path when the request fails", async () => {
    vi.mocked(fetch).mockImplementation((input, init) => {
      const rawPath =
        typeof input === "string"
          ? input
          : input instanceof URL
            ? input.toString()
            : input.url;
      const path = new URL(rawPath, "http://localhost").pathname;

      if (path === "/api/memory/dream/jobs") {
        return Promise.resolve(jsonResponse({ error: "Dream history request failed" }, { status: 502 }));
      }

      return Promise.resolve(mockFetch(input, init));
    });
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Dream history request failed")).toBeInTheDocument();
    expect(screen.queryByText("Some remote Dream history is unavailable")).toBeNull();
  });

  it("requests Dream history pages from the server", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    const dreamHistory = (await screen.findByText("Dream history")).closest("section");
    if (!dreamHistory) {
      throw new Error("Expected Dream history section");
    }
    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(([input]) => {
          const url = new URL(String(input), "http://localhost");
          return (
            url.pathname === "/api/memory/dream/jobs" &&
            url.searchParams.get("page") === "1" &&
            url.searchParams.get("pageSize") === "10"
          );
        }),
      ).toBe(true);
    });

    changeInput(within(dreamHistory).getByLabelText("Page size"), "1");
    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(([input]) => {
          const url = new URL(String(input), "http://localhost");
          return (
            url.pathname === "/api/memory/dream/jobs" &&
            url.searchParams.get("page") === "1" &&
            url.searchParams.get("pageSize") === "1"
          );
        }),
      ).toBe(true);
    });

    await userEvent.click(within(dreamHistory).getByRole("button", { name: "Next page" }));
    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(([input]) => {
          const url = new URL(String(input), "http://localhost");
          return (
            url.pathname === "/api/memory/dream/jobs" &&
            url.searchParams.get("page") === "2" &&
            url.searchParams.get("pageSize") === "1"
          );
        }),
      ).toBe(true);
    });
  });
  it("shows Dream history actions and runs manual Dream jobs", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    expect(await screen.findByText("Dream history")).toBeInTheDocument();
    const dreamTable = screen.getByRole("table");
    expect(within(dreamTable).getByRole("columnheader", { name: "Actions" })).toBeInTheDocument();
    const dreamTableScroller = dreamTable.parentElement;
    const settingsScroller = dreamTable.closest(".settings-content-scroll") as HTMLElement | null;
    expect(dreamTableScroller).toHaveClass("overflow-x-auto");
    expect(dreamTableScroller).toHaveClass("settings-table-scroll");
    expect(settingsScroller).not.toBeNull();
    if (!dreamTableScroller || !settingsScroller) {
      throw new Error("Expected Dream history to live inside settings content scroller");
    }
    settingsScroller.style.overflowY = "auto";
    Object.defineProperties(settingsScroller, {
      clientHeight: { configurable: true, value: 360 },
      scrollHeight: { configurable: true, value: 960 },
    });
    settingsScroller.scrollTop = 0;
    fireEvent.wheel(dreamTableScroller, { deltaX: 0, deltaY: 120 });
    expect(settingsScroller.scrollTop).toBe(120);
    settingsScroller.scrollTop = 0;
    const verticalTouchMove = new Event("touchmove", {
      bubbles: true,
      cancelable: true,
    });
    Object.defineProperty(verticalTouchMove, "touches", {
      value: [{ clientX: 24, clientY: 90 }],
    });
    dreamTableScroller.dispatchEvent(verticalTouchMove);
    expect(verticalTouchMove.defaultPrevented).toBe(false);
    expect(settingsScroller.scrollTop).toBe(0);
    expect(within(dreamTable).getAllByText(workspace.name)).toHaveLength(2);
    expect(within(dreamTable).getAllByRole("button", { name: "View details" })).toHaveLength(2);
    expect(screen.queryByText(memoryDreamJob.summary!)).not.toBeInTheDocument();
    expect(screen.queryByText(memoryDreamChange.reason)).not.toBeInTheDocument();
    expect(screen.queryByText("Before JSON")).not.toBeInTheDocument();
    expect(
      workspace.chats.some((chat) => chat.id === memoryDreamJob.transcriptChatId),
    ).toBe(false);
    expect(screen.getByRole("button", { name: "Open transcript" })).toBeInTheDocument();

    const dreamRows = within(dreamTable).getAllByRole("row");
    await userEvent.click(within(dreamRows[2]).getByText(workspace.name));
    expect(screen.queryByRole("dialog", { name: "Dream job details" })).not.toBeInTheDocument();

    await userEvent.click(within(dreamRows[2]).getByRole("button", { name: "View details" }));
    const failedDreamDialog = await screen.findByRole("dialog", {
      name: "Dream job details",
    });
    expect(
      within(failedDreamDialog).getByText(failedMemoryDreamJob.errorMessage!),
    ).toBeInTheDocument();
    await userEvent.click(
      within(failedDreamDialog).getByRole("button", { name: "Close Dream job details" }),
    );
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Dream job details" })).not.toBeInTheDocument();
    });

    await userEvent.click(within(dreamRows[1]).getByRole("button", { name: "View details" }));
    const dreamDialog = await screen.findByRole("dialog", { name: "Dream job details" });
    expect(within(dreamDialog).getByText(memoryDreamJob.summary!)).toBeInTheDocument();
    expect(await within(dreamDialog).findByText(memoryDreamChange.reason)).toBeInTheDocument();
    expect(within(dreamDialog).getByText("Before JSON")).toBeInTheDocument();
    expect(
      within(dreamDialog).getByText("Memory state before this Dream change."),
    ).toBeInTheDocument();
    expect(within(dreamDialog).getByText("After JSON")).toBeInTheDocument();
    expect(
      within(dreamDialog).getByText("Memory state Dream wrote or proposed."),
    ).toBeInTheDocument();
    expect(within(dreamDialog).getByText("Evidence JSON")).toBeInTheDocument();
    expect(
      within(dreamDialog).getByText("Sources Dream used to justify the change."),
    ).toBeInTheDocument();
    await userEvent.click(
      within(dreamDialog).getByRole("button", { name: "Close Dream job details" }),
    );
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Dream job details" })).not.toBeInTheDocument();
    });

    await userEvent.click(screen.getByLabelText("Enable memory"));
    await userEvent.click(screen.getByLabelText("Enable Dream"));
    const dreamJobCallsBeforeRun = fetchMock.mock.calls.filter(([input]) => {
      const url = new URL(String(input), "http://localhost");
      return url.pathname === "/api/memory/dream/jobs";
    }).length;
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
      expect(
        fetchMock.mock.calls.filter(([input]) => {
          const url = new URL(String(input), "http://localhost");
          return url.pathname === "/api/memory/dream/jobs";
        }).length,
      ).toBeGreaterThan(dreamJobCallsBeforeRun);
    });

    await userEvent.click(screen.getByRole("button", { name: "Open transcript" }));
    expect(await screen.findByText(/job started/)).toBeInTheDocument();
    expect(screen.queryByText("API overview")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Send message" })).not.toBeInTheDocument();
  });

  it("polls Dream history until an active manual run reaches a terminal status", async () => {
    const runningDreamJob: MemoryDreamJobSummary = {
      ...(memoryDreamJob as MemoryDreamJobSummary),
      changeCounts: {
        added: 0,
        expired: 0,
        rejected: 0,
        superseded: 0,
        updated: 0,
      },
      completedAt: null,
      status: "running",
      summary: null,
    };
    const completedDreamJob = memoryDreamJob as MemoryDreamJobSummary;
    const terminalFailedDreamJob = failedMemoryDreamJob as MemoryDreamJobSummary;
    appTestState.memoryDreamJobsResponses = [
      {
        jobs: [completedDreamJob, terminalFailedDreamJob],
        page: 1,
        pageSize: 10,
        totalCount: 2,
        totalPages: 1,
      },
      {
        jobs: [runningDreamJob, terminalFailedDreamJob],
        page: 1,
        pageSize: 10,
        totalCount: 2,
        totalPages: 1,
      },
      {
        jobs: [terminalFailedDreamJob, completedDreamJob],
        page: 1,
        pageSize: 10,
        totalCount: 2,
        totalPages: 1,
      },
    ];
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));
    expect(await screen.findByText("Dream history")).toBeInTheDocument();
    await waitFor(() => {
      expect(
        fetchMock.mock.calls.filter(([input]) => {
          const url = new URL(String(input), "http://localhost");
          return url.pathname === "/api/memory/dream/jobs";
        }).length,
      ).toBeGreaterThanOrEqual(1);
    });

    await userEvent.click(screen.getByLabelText("Enable memory"));
    await userEvent.click(screen.getByLabelText("Enable Dream"));

    let dreamPoll: (() => void) | null = null;
    const originalSetInterval = window.setInterval.bind(window);
    const pollIntervalId = 7300 as unknown as ReturnType<typeof window.setInterval>;
    const clearIntervalSpy = vi.spyOn(window, "clearInterval");
    vi.spyOn(window, "setInterval").mockImplementation(((
      ...args: Parameters<typeof window.setInterval>
    ) => {
      const [handler, timeout] = args;
      if (timeout === 3000 && typeof handler === "function") {
        dreamPoll = () => handler();
        return pollIntervalId;
      }
      return (originalSetInterval as (...intervalArgs: unknown[]) => unknown)(
        ...args,
      ) as ReturnType<typeof window.setInterval>;
    }) as typeof window.setInterval);

    await userEvent.click(screen.getByRole("button", { name: "Run workspace Dream now" }));
    expect(await screen.findByText("Running")).toBeInTheDocument();
    await waitFor(() => {
      expect(dreamPoll).not.toBeNull();
      expect(
        fetchMock.mock.calls.filter(([input]) => {
          const url = new URL(String(input), "http://localhost");
          return url.pathname === "/api/memory/dream/jobs";
        }).length,
      ).toBeGreaterThanOrEqual(2);
    });

    const poll = dreamPoll as (() => void) | null;
    if (!poll) {
      throw new Error("Expected Dream history polling interval to be registered");
    }
    await act(async () => {
      poll();
    });

    expect(await screen.findByText("Failed")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.queryByText("Running")).not.toBeInTheDocument();
      expect(clearIntervalSpy).toHaveBeenCalledWith(pollIntervalId);
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

  it("toggles memory enabled state per row without changing pin or list membership", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    const disableLabel = `Disable memory ${activeMemory.fact}`;
    const enableLabel = `Enable memory ${activeMemory.fact}`;
    const disableSwitch = await screen.findByRole("switch", { name: disableLabel });
    expect(disableSwitch).toBeChecked();
    expect(activeMemory.pinned).toBe(true);

    await userEvent.click(disableSwitch);

    await waitFor(() => {
      const enabledCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/memory/enabled",
      );
      expect(enabledCall).toBeDefined();
      expect(JSON.parse(String(enabledCall?.[1]?.body))).toEqual({
        chatId: null,
        enabled: false,
        factId: activeMemory.id,
        scope: "global",
        workspaceId: null,
      });
    });

    const enableSwitch = await screen.findByRole("switch", { name: enableLabel });
    expect(enableSwitch).not.toBeChecked();
    expect(screen.getAllByText(activeMemory.fact).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Edit memory" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Delete memory" })).toBeEnabled();

    await userEvent.click(enableSwitch);
    expect(await screen.findByRole("switch", { name: disableLabel })).toBeChecked();
    expect(appTestState.memoriesById[activeMemory.id].pinned).toBe(true);
  });

  it("keeps only the target memory toggle pending and preserves state on failure", async () => {
    const secondMemory = {
      ...activeMemory,
      fact: "Another active memory",
      id: "memory-active-2",
      pinned: false,
    };
    appTestState.memoriesById[secondMemory.id] = secondMemory;
    appTestState.memoryListAdditional = [secondMemory];
    const pendingResponse = deferred<Response>();
    appTestState.memoryEnabledResponses.push(pendingResponse.promise);
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Memory" }));

    const activeDisableLabel = `Disable memory ${activeMemory.fact}`;
    const activeSwitch = await screen.findByRole("switch", { name: activeDisableLabel });
    await userEvent.click(activeSwitch);

    await waitFor(() => expect(activeSwitch).toBeDisabled());
    const secondSwitch = await screen.findByRole("switch", {
      name: `Disable memory ${secondMemory.fact}`,
    });
    expect(secondSwitch).toBeEnabled();
    expect(secondSwitch).toBeChecked();

    pendingResponse.resolve(jsonResponse({ error: "toggle failed" }, { status: 500 }));

    await waitFor(() => {
      expect(fetchMock.mock.calls.filter(([url]) => url === "/api/memory/enabled")).toHaveLength(1);
      expect(screen.getByText("toggle failed")).toBeInTheDocument();
    });

    const restoredSwitch = screen.getByRole("switch", { name: activeDisableLabel });
    expect(restoredSwitch).toBeChecked();
    expect(restoredSwitch).toBeEnabled();
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

  it("defers workspace hooks loading until the Hooks settings section is opened", async () => {
    appTestState.settingsResponse = {
      ...settings,
      workspaces: [
        settings.workspaces[0],
        {
          commonCommands: secondaryWorkspace.commonCommands,
          connectionStatus: secondaryWorkspace.connectionStatus,
          displayPath: secondaryWorkspace.displayPath,
          id: secondaryWorkspace.id,
          isDefault: false,
          lastRemoteError: secondaryWorkspace.lastRemoteError,
          logoUrl: secondaryWorkspace.logoUrl,
          name: secondaryWorkspace.name,
          path: secondaryWorkspace.path,
          pinned: secondaryWorkspace.pinned,
          remotePath: secondaryWorkspace.remotePath,
          serverId: secondaryWorkspace.serverId,
          serverName: secondaryWorkspace.serverName,
          terminalShell: secondaryWorkspace.terminalShell,
        },
      ],
    };
    const fetchMock = vi.mocked(fetch);
    const hooksFetchCalls = () =>
      fetchMock.mock.calls.filter(([url]) => {
        const value = String(url);
        return value === "/api/hooks" || value.startsWith("/api/hooks?");
      });

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    expect(await screen.findByText("General settings")).toBeInTheDocument();
    // Wait until /api/settings has driven form state (including hookWorkspaceId),
    // so this assertion cannot race past an ungated hooks effect.
    expect(await screen.findByText("127.0.0.1:3210")).toBeInTheDocument();
    expect(screen.getByText("Password is disabled")).toBeInTheDocument();

    await waitFor(() => {
      expect(hooksFetchCalls()).toEqual([]);
    });

    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    const hooksCallsBeforeSection = hooksFetchCalls().length;
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Hooks" }));

    expect(await screen.findByText("Hook settings")).toBeInTheDocument();
    await waitFor(() => {
      expect(hooksFetchCalls()).toHaveLength(hooksCallsBeforeSection + 1);
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/hooks?workspaceId=workspace-1",
        expect.anything(),
      );
    });
    expect(screen.getByText("Record hook run logs")).toBeInTheDocument();

    const hooksCallsBeforeWorkspaceSwitch = hooksFetchCalls().length;
    await userEvent.selectOptions(screen.getByLabelText("Workspace"), secondaryWorkspace.id);

    await waitFor(() => {
      expect(hooksFetchCalls()).toHaveLength(hooksCallsBeforeWorkspaceSwitch + 1);
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/hooks?workspaceId=workspace-2",
        expect.anything(),
      );
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

    await userEvent.click(screen.getAllByRole("button", { name: /Pre tool use/ }).at(-1)!);
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
    changeInput(screen.getByTestId("review-system-prompt"), "Review as senior engineer.");
    await userEvent.type(
      screen.getByLabelText("Prompt file path"),
      "C:/Users/fonla/.codex/AGENTS.md",
    );
    await userEvent.click(screen.getByRole("button", { name: "Add prompt file" }));
    await userEvent.type(screen.getByLabelText("Extra prompt"), "Keep replies concise.");
    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(([url]) => url === "/api/settings/prompts");
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toEqual({
        contextCompressionSystemPrompt: null,
        generationSystemPrompt: null,
        updateSystemPrompt: null,
        memoryRetrievalSystemPrompt: null,
        memoryExtractionSystemPrompt: null,
        memoryDreamSystemPrompt: null,
        extraText: "Keep replies concise.",
        files: ["C:/Users/fonla/.codex/AGENTS.md"],
        systemPrompts: [
          {
            content: "Custom system prompt.",
            name: "Default",
          },
          {
            content: defaultPlanModeSystemPrompt,
            name: "Plan Mode",
          },
          {
            content: "Review as senior engineer.",
            name: "Review",
          },
        ],
      });
    });
  });

  it("opens the prompt file picker from prompt settings", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));

    const choosePromptFileButton = screen.getByRole("button", { name: "Choose prompt file" });
    await userEvent.click(choosePromptFileButton);

    const dialog = await screen.findByRole("dialog", { name: "Select prompt file" });
    expect(dialog).toBeInTheDocument();

    fireEvent.keyDown(dialog, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "Select prompt file" })).not.toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: "Choose prompt file" })).not.toBeDisabled();
  });

  it("hides the built-in image agent role prompt from prompt settings", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));

    const defaultPromptButton = screen.getByRole("button", { name: "Default" });
    expect(defaultPromptButton).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Plan Mode" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Review" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Image Generation" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("Plan Mode prompt")).toBeInTheDocument();
    expect(screen.getByLabelText("Review Agent prompt")).toBeInTheDocument();
    const restoreButtons = screen.getAllByRole("button", {
      name: "Restore default system prompt",
    });
    expect(restoreButtons).toHaveLength(1);
  });

  it("keeps built-in Review fixed while renaming user system prompts", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));

    expect(
      screen.queryByRole("button", { name: "Rename system prompt Default" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Rename system prompt Plan Mode" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Rename system prompt Review" }),
    ).not.toBeInTheDocument();

    await userEvent.type(screen.getByPlaceholderText("Prompt name"), "Reviewer Draft");
    await userEvent.click(screen.getByRole("button", { name: "Add system prompt" }));
    const renameButton = screen.getByRole("button", {
      name: "Rename system prompt Reviewer Draft",
    });
    expect(renameButton).toBeInTheDocument();

    await userEvent.type(screen.getByLabelText("System prompt"), "Review as senior engineer.");
    await userEvent.click(renameButton);
    const nameInput = screen.getByRole("textbox", { name: "System prompt name" });
    await userEvent.clear(nameInput);
    await userEvent.type(nameInput, "Reviewer");
    await userEvent.click(screen.getByRole("button", { name: "Save system prompt name" }));
    expect(screen.getByRole("button", { name: "Reviewer" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(([url]) => url === "/api/settings/prompts");
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toEqual({
        contextCompressionSystemPrompt: null,
        generationSystemPrompt: null,
        updateSystemPrompt: null,
        memoryRetrievalSystemPrompt: null,
        memoryExtractionSystemPrompt: null,
        memoryDreamSystemPrompt: null,
        extraText: "",
        files: [],
        systemPrompts: [
          {
            content: "You are Foco, a local coding agent.",
            name: "Default",
          },
          {
            content: defaultPlanModeSystemPrompt,
            name: "Plan Mode",
          },
          {
            content: defaultReviewSystemPrompt,
            name: "Review",
          },
          {
            name: "Reviewer",
            content: "Review as senior engineer.",
          },
        ],
      });
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
    await userEvent.click(
      screen.getByRole("button", { name: "Restore default system prompt" }),
    );
    expect(systemPromptInput).toHaveValue("You are Foco, a local coding agent.");

    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(([url]) => url === "/api/settings/prompts");
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toEqual({
        contextCompressionSystemPrompt: null,
        generationSystemPrompt: null,
        updateSystemPrompt: null,
        memoryRetrievalSystemPrompt: null,
        memoryExtractionSystemPrompt: null,
        memoryDreamSystemPrompt: null,
        extraText: "",
        files: [],
        systemPrompts: [
          {
            content: "You are Foco, a local coding agent.",
            name: "Default",
          },
          {
            content: defaultPlanModeSystemPrompt,
            name: "Plan Mode",
          },
          {
            content: defaultReviewSystemPrompt,
            name: "Review",
          },
        ],
      });
    });
  });

  it("renders prompt override editors in the configured order", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));

    const order = [
      screen.getByLabelText("Extra prompt"),
      screen.getByLabelText("System prompt"),
      screen.getByLabelText("Plan Mode prompt"),
      screen.getByLabelText("Review Agent prompt"),
      screen.getByLabelText("Context compression prompt"),
      screen.getByLabelText("Spec generation prompt"),
      screen.getByLabelText("Spec update prompt"),
      screen.getByLabelText("Memory matching prompt"),
      screen.getByLabelText("Memory extraction prompt"),
      screen.getByLabelText("Dream prompt"),
      screen.getByLabelText("Prompt file path"),
    ];
    for (const [index, control] of order.entries()) {
      const nextControl = order[index + 1];
      if (nextControl) {
        expect(
          control.compareDocumentPosition(nextControl) & Node.DOCUMENT_POSITION_FOLLOWING,
        ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      }
    }
  });

  it("loads, edits, restores, and saves context compression prompt", async () => {
    const fetchMock = vi.mocked(fetch);
    const defaultCompressionPrompt =
      "You are creating a context checkpoint handoff summary for a coding agent so work can continue after older conversation messages are replaced by this summary.";
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      prompts: {
        ...appTestState.settingsResponse.prompts,
        contextCompressionSystemPrompt: "Custom checkpoint handoff.",
        defaultContextCompressionSystemPrompt: defaultCompressionPrompt,
      },
    };
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));
    await waitFor(() => {
      expect(screen.getByLabelText("System prompt")).toHaveValue(
        "You are Foco, a local coding agent.",
      );
    });

    await waitFor(() => {
      expect(screen.getByTestId("context-compression-system-prompt")).toHaveValue(
        "Custom checkpoint handoff.",
      );
    });
    const compressionInput = screen.getByTestId("context-compression-system-prompt");
    changeInput(compressionInput, "Edited checkpoint prompt.");
    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/prompts",
        expect.objectContaining({
          body: expect.stringContaining(
            '"contextCompressionSystemPrompt":"Edited checkpoint prompt."',
          ),
          method: "POST",
        }),
      );
    });

    await userEvent.click(
      screen.getByRole("button", { name: "Restore default context compression prompt" }),
    );
    expect(compressionInput).toHaveValue(defaultCompressionPrompt);
    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/settings/prompts",
        expect.objectContaining({
          body: expect.stringContaining('"contextCompressionSystemPrompt":null'),
          method: "POST",
        }),
      );
    });
  });

  it("keeps context compression prompt edits when save fails", async () => {
    const fetchMock = vi.mocked(fetch);
    fetchMock.mockImplementation(async (input, init) => {
      const path = typeof input === "string" ? input : input.toString();
      if (path === "/api/settings/prompts" && init?.method === "POST") {
        return new Response(JSON.stringify({ error: "prompt save failed" }), {
          status: 500,
          headers: { "Content-Type": "application/json" },
        });
      }
      return mockFetch(input, init);
    });
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Prompts" }));
    await waitFor(() => {
      expect(screen.getByLabelText("System prompt")).toHaveValue(
        "You are Foco, a local coding agent.",
      );
    });

    const compressionInput = screen.getByTestId("context-compression-system-prompt");
    changeInput(compressionInput, "Unsaved compression prompt.");
    await userEvent.click(screen.getByRole("button", { name: "Save prompt settings" }));

    await waitFor(() => {
      expect(screen.getByText(/prompt save failed/i)).toBeInTheDocument();
    });
    expect(compressionInput).toHaveValue("Unsaved compression prompt.");
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

  it("shows compact model rows with system prompt names and confirms deletion", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Models" }));

    expect(await screen.findByText("GPT Test")).toBeInTheDocument();
    expect(screen.getByText("system prompt Default")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Disable model GPT Test" })).toBeChecked();
    expect(
      screen.getByRole("button", { name: "Edit model GPT Test" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Delete model GPT Test" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("limits ok")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Delete model GPT Test" }));

    expect(confirmSpy).toHaveBeenCalledWith("Delete model confirmation");
    expect(fetchMock.mock.calls.some(([url]) => url === "/api/models/delete")).toBe(
      false,
    );
    confirmSpy.mockRestore();
  });
  it("prefills model details from the selected developer metadata", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];

      if (path === "/api/model-metadata") {
        return jsonResponse({
          cachePath: "C:\\Users\\fonla\\.foco\\models.dev.json",
          configuredModels: settings.configuredModels,
          fetchedAt: "2026-06-05T10:00:00Z",
          models: [
            {
              contextWindow: 200000,
              inputModalities: ["text", "image", "pdf"],
              key: "openai/o3",
              maxOutputTokens: 100000,
              modelId: "openai/o3",
              name: "o3",
              outputModalities: ["text"],
              pricing: {
                cacheRead: 0.5,
                cacheWrite: null,
                input: 2,
                output: 8,
                reasoning: null,
              },
              providerId: "openai",
              providerName: "OpenAI",
              reasoning: true,
              refreshedAt: "2026-06-05T10:00:00Z",
              sourceUrl: "https://models.dev/api.json",
              supportedThinkingLevels: ["low", "high"],
              supportsCache: true,
              supportsTools: true,
            },
            {
              contextWindow: 200000,
              inputModalities: ["text"],
              key: "anthropic/claude-test",
              maxOutputTokens: 64000,
              modelId: "claude-test",
              name: "Claude Test",
              outputModalities: ["text"],
              pricing: {
                cacheRead: null,
                cacheWrite: null,
                input: 5,
                output: 25,
                reasoning: null,
              },
              providerId: "anthropic",
              providerName: "Anthropic",
              reasoning: false,
              refreshedAt: "2026-06-05T10:00:00Z",
              sourceUrl: "https://models.dev/api.json",
              supportedThinkingLevels: [],
              supportsCache: false,
              supportsTools: true,
            },
          ],
          sourceUrl: "https://models.dev/api.json",
        });
      }

      if (path === "/api/providers/models") {
        const body = JSON.parse(String(init?.body ?? "{}")) as { providerId?: string };
        return jsonResponse({
          providerId: body.providerId,
          models: body.providerId === "openai" ? ["o3"] : [],
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Providers" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Load provider models for OpenAI" }),
    );
    await screen.findByText("o3");

    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Add model" }));
    await userEvent.selectOptions(screen.getByLabelText("Model developer"), "openai");
    const modelIdSelect = screen.getByLabelText("Model id");
    await userEvent.click(modelIdSelect);
    expect(screen.getByRole("option", { name: "o3" })).toBeInTheDocument();
    expect(
      screen.queryByRole("option", { name: "claude-test" }),
    ).toBeNull();

    await userEvent.click(screen.getByRole("option", { name: "o3" }));

    expect(screen.getByLabelText("Display name")).toHaveValue("o3");
    expect(screen.getByLabelText("Context window")).toHaveValue("200000");
    const inputTypes = screen.getByRole("group", { name: "Input types" });
    expect(within(inputTypes).getByRole("checkbox", { name: "image" })).toBeChecked();
    await expectSelectedOption(screen.getByLabelText("Thinking level"), "low");
    expect(screen.getByText("openai/o3")).toBeInTheDocument();
    expect(screen.getByText("$2")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "OpenAI" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Anthropic" })).toBeDisabled();
    expect(screen.getByText("Model not supported")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Save model" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/models/manual",
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(saveCall![1]?.body as string)).toMatchObject({
        contextWindow: 200000,
        maxOutputTokens: 100000,
        metadataKey: "openai/o3",
        modelId: "o3",
        providerIds: ["openai"],
        activeProviderId: "openai",
        thinkingLevel: "low",
      });
    });
  });
  it("disables unsupported providers when editing a metadata-backed model", async () => {
    appTestState.settingsResponse = {
      ...settings,
      configuredModels: [
        {
          ...settings.configuredModels[0],
          activeProviderId: "openai",
          displayName: "Created Model",
          id: "created-model",
          metadataKey: "openai/created-model",
          providerIds: ["openai"],
          supportsThinking: false,
          thinkingLevel: null,
        },
      ],
    };

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Edit model Created Model" }));

    expect(screen.getByRole("checkbox", { name: "OpenAI" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Anthropic" })).toBeDisabled();
    await expectSelectedOption(screen.getByLabelText("Active provider"), "openai");
  });

  it("keeps configured provider associations when local provider id differs from metadata", async () => {
    const geminiProvider = {
      ...settings.providers[1],
      id: "gemini-openrouter",
      kindLabel: "Gemini",
      name: "Gemini Router",
    };
    appTestState.settingsResponse = {
      ...settings,
      configuredModels: [
        {
          ...settings.configuredModels[0],
          activeProviderId: geminiProvider.id,
          displayName: "Gemini 3.5 Flash",
          id: "gemini-3.5-flash",
          metadataKey: "google/gemini-3.5-flash",
          providerIds: [geminiProvider.id],
          supportsThinking: false,
          thinkingLevel: null,
        },
      ],
      providers: [settings.providers[0], geminiProvider],
    };

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Edit model Gemini 3.5 Flash" }));

    expect(screen.getByRole("checkbox", { name: "OpenAI" })).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: "Gemini Router" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Gemini Router" })).not.toBeDisabled();
    await expectSelectedOption(screen.getByLabelText("Active provider"), geminiProvider.id);
  });

  it("auto-loads provider models before matching a metadata model to local provider ids", async () => {
    const geminiProvider = {
      ...settings.providers[1],
      id: "gemini-openrouter",
      kindLabel: "Gemini",
      name: "Gemini Router",
    };
    appTestState.settingsResponse = {
      ...settings,
      providers: [settings.providers[0], geminiProvider],
    };
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];

      if (path === "/api/model-metadata") {
        return jsonResponse({
          cachePath: "C:\\Users\\fonla\\.foco\\models.dev.json",
          configuredModels: [],
          fetchedAt: "2026-06-05T10:00:00Z",
          models: [
            {
              contextWindow: 1000000,
              inputModalities: ["text"],
              key: "google/gemini-3.5-flash",
              maxOutputTokens: 65536,
              modelId: "gemini-3.5-flash",
              name: "Gemini 3.5 Flash",
              outputModalities: ["text"],
              pricing: {
                cacheRead: null,
                cacheWrite: null,
                input: 0.3,
                output: 2.5,
                reasoning: null,
              },
              providerId: "google",
              providerName: "Google",
              reasoning: false,
              refreshedAt: "2026-06-05T10:00:00Z",
              sourceUrl: "https://models.dev/api.json",
              supportedThinkingLevels: [],
              supportsCache: false,
              supportsTools: true,
            },
          ],
          sourceUrl: "https://models.dev/api.json",
        });
      }

      if (path === "/api/providers/models") {
        const body = JSON.parse(String(init?.body ?? "{}")) as { providerId?: string };
        return jsonResponse({
          providerId: body.providerId,
          models:
            body.providerId === geminiProvider.id ? ["gemini-3.5-flash"] : [],
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Add model" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url, init]) =>
            url === "/api/providers/models" &&
            JSON.parse(String(init?.body ?? "{}"))?.providerId === geminiProvider.id,
        ),
      ).toBe(true);
    });

    await userEvent.selectOptions(screen.getByLabelText("Model developer"), "google");
    await userEvent.selectOptions(screen.getByLabelText("Model id"), "gemini-3.5-flash");

    await waitFor(() => {
      expect(screen.getByRole("checkbox", { name: "Gemini Router" })).toBeChecked();
    });
    expect(screen.getByRole("checkbox", { name: "OpenAI" })).toBeDisabled();

    await userEvent.click(screen.getByRole("button", { name: "Save model" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/models/manual",
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(saveCall![1]?.body as string)).toMatchObject({
        activeProviderId: geminiProvider.id,
        modelId: "gemini-3.5-flash",
        providerIds: [geminiProvider.id],
      });
    });
  });

  it("keeps thinking level editable when configured model supports it despite stale metadata", async () => {
    const gpt55Model = {
      ...settings.configuredModels[0],
      displayName: "GPT 5.5",
      id: "gpt-5.5",
      metadataKey: "openai/gpt-5.5",
      supportsThinking: true,
      thinkingLevel: "high",
    };
    appTestState.settingsResponse = {
      ...settings,
      configuredModels: [gpt55Model],
    };

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Edit model GPT 5.5" }));

    const thinkingLevel = screen.getByLabelText("Thinking level");
    // ponytail: UI-level regression check; save behavior is already covered by model form tests.
    expect(thinkingLevel).not.toBeDisabled();
    await expectSelectedOption(thinkingLevel, "high");
  });

  it("toggles configured models from the model list", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Disable model GPT Test" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/models/manual",
        expect.objectContaining({
          body: expect.stringContaining('"enabled":false'),
          method: "POST",
        }),
      );
    });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/models/manual",
      expect.objectContaining({
        body: expect.stringContaining('"modelId":"gpt-test"'),
        method: "POST",
      }),
    );
  });

  it("localizes provider list actions", async () => {
    const zhSettings = {
      ...settings,
      general: { ...settings.general, language: "zh-CN" as const },
    };
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = typeof input === "string" ? input : input.toString();
        const path = url.startsWith("http://127.0.0.1")
          ? new URL(url).pathname
          : url.split("?")[0];
        return path === "/api/settings" ? jsonResponse(zhSettings) : mockFetch(input, init);
      }),
    );

    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "设置" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "设置" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "供应商" }));

    expect(screen.getByRole("checkbox", { name: "停用供应商 OpenAI" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除供应商 OpenAI" })).toBeInTheDocument();
  });

  it("toggles configured providers from the provider list without dropping settings", async () => {
    const fetchMock = vi.mocked(fetch);
    const openAiProvider = {
      ...settings.providers[0],
      apiProxy: {
        ...settings.providers[0].apiProxy,
        enabled: true,
        proxyType: "socks",
        url: "127.0.0.1:7891",
      },
      modelRedirects: [{ from: "upstream-model", to: "local-model" }],
      requestOverrides: [
        {
          target: "body" as const,
          name: "text.verbosity",
          valueType: "string" as const,
          value: "low",
        },
      ],
    };
    appTestState.settingsResponse = {
      ...settings,
      providers: [
        openAiProvider as ConfiguredProviderSummary &
          (typeof appTestState.settingsResponse.providers)[number],
        settings.providers[1],
      ],
    };
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));

    const toggle = screen.getByRole("checkbox", {
      name: "Disable provider OpenAI",
    });
    expect(toggle).toHaveAccessibleName("Disable provider OpenAI");
    expect(toggle).toBeChecked();

    await userEvent.click(toggle);

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url, init]) =>
          url === "/api/providers/manual" &&
          JSON.parse(String(init?.body ?? "{}")).id === "openai" &&
          JSON.parse(String(init?.body ?? "{}")).enabled === false,
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toEqual({
        apiKey: null,
        apiProxy: {
          enabled: openAiProvider.apiProxy.enabled,
          proxyType: openAiProvider.apiProxy.proxyType,
          url: openAiProvider.apiProxy.url,
        },
        baseUrl: openAiProvider.baseUrl,
        clearApiKey: false,
        enabled: false,
        id: openAiProvider.id,
        kind: openAiProvider.kind,
        autoSyncModels: openAiProvider.autoSyncModels,
        modelSyncFilterRegex: openAiProvider.modelSyncFilterRegex,
        modelRedirects: openAiProvider.modelRedirects,
        name: openAiProvider.name,
        requestOverrides: openAiProvider.requestOverrides,
      });
    });
    await waitFor(() => {
      expect(
        screen.getByRole("checkbox", { name: "Enable provider OpenAI" }),
      ).not.toBeChecked();
    });
  });

  it("keeps provider list operations scoped to the active row", async () => {
    const pendingResponse = deferred<Response>();
    const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      if (url === "/api/providers/manual") {
        return pendingResponse.promise;
      }
      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));

    const openAiToggle = screen.getByRole("checkbox", { name: "Disable provider OpenAI" });
    const anthropicToggle = screen.getByRole("checkbox", {
      name: "Disable provider Anthropic",
    });
    const openAiDelete = screen.getByRole("button", { name: "Delete provider OpenAI" });
    const anthropicDelete = screen.getByRole("button", {
      name: "Delete provider Anthropic",
    });

    fireEvent.click(openAiToggle);
    fireEvent.click(openAiToggle);

    await waitFor(() => expect(openAiToggle).toBeDisabled());
    expect(openAiDelete).toBeDisabled();
    expect(anthropicToggle).toBeEnabled();
    expect(anthropicDelete).toBeEnabled();
    expect(fetchMock.mock.calls.filter(([url]) => url === "/api/providers/manual")).toHaveLength(1);

    pendingResponse.resolve(jsonResponse(settings));
    await waitFor(() => expect(openAiToggle).toBeEnabled());
  });

  it("deletes configured providers and clears their expanded model cache", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));

    await userEvent.click(
      screen.getByRole("button", { name: "Load provider models for OpenAI" }),
    );
    expect(await screen.findByText("gpt-4.1")).toBeInTheDocument();

    const deleteButton = screen.getByRole("button", { name: "Delete provider OpenAI" });
    expect(deleteButton).toHaveAccessibleName("Delete provider OpenAI");
    await userEvent.click(deleteButton);

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/providers/delete",
        expect.objectContaining({
          body: JSON.stringify({ id: "openai" }),
          method: "POST",
        }),
      );
    });
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Delete provider OpenAI" })).toBeNull();
      expect(screen.queryByText("gpt-4.1")).toBeNull();
    });

    await userEvent.click(screen.getByRole("button", { name: "Add provider" }));
    changeInput(screen.getByLabelText("Name"), "OpenAI");
    await userEvent.click(screen.getByRole("button", { name: "Save provider" }));

    const restoredOpenAiRow = await screen.findByRole("button", {
      name: "Load provider models for OpenAI",
    });
    expect(restoredOpenAiRow).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("gpt-4.1")).toBeNull();
  });

  it("keeps provider enable and delete controls out of the edit dialog", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    await userEvent.click(screen.getByRole("button", { name: "Edit provider OpenAI" }));

    const providerDialog = screen.getByRole("form", { name: "Provider configuration" });
    expect(within(providerDialog).queryByRole("checkbox", { name: /provider OpenAI/ })).toBeNull();
    expect(within(providerDialog).queryByRole("button", { name: /Delete provider/ })).toBeNull();
    const closeButton = within(providerDialog).getByRole("button", {
      name: "Close provider configuration",
    });
    expect(closeButton).toBeInTheDocument();
    await userEvent.click(within(providerDialog).getByRole("button", { name: "Save provider" }));
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/providers/manual",
        expect.objectContaining({ method: "POST" }),
      );
      expect(screen.queryByRole("form", { name: "Provider configuration" })).toBeNull();
    });

    await userEvent.click(screen.getByRole("button", { name: "Edit provider OpenAI" }));
    await userEvent.click(
      screen.getByRole("button", { name: "Close provider configuration" }),
    );
    expect(screen.queryByRole("form", { name: "Provider configuration" })).toBeNull();
  });

  it("saves provider model redirects from the provider form", async () => {
    const fetchMock = vi.mocked(fetch);
    appTestState.settingsResponse = {
      ...settings,
      providers: [
        {
          ...settings.providers[0],
          modelRedirects: [
            { from: "qwen/qwen3.6-35b-a3b", to: "qwen3.6-35b-a3b" },
          ],
        },
        settings.providers[1],
      ],
    };
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));

    expect(screen.getByText("qwen/qwen3.6-35b-a3b -> qwen3.6-35b-a3b")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Edit provider OpenAI" }));
    expect(screen.getByLabelText("Upstream model")).toHaveValue("qwen/qwen3.6-35b-a3b");
    expect(screen.getByLabelText("Local model")).toHaveValue("qwen3.6-35b-a3b");

    await userEvent.click(screen.getByRole("button", { name: "Add redirect" }));
    const upstreamInputs = screen.getAllByLabelText("Upstream model");
    const localInputs = screen.getAllByLabelText("Local model");
    changeInput(upstreamInputs[1], "deepseek/deepseek-r1");
    expect(screen.getByRole("button", { name: "Save provider" })).toBeDisabled();
    changeInput(localInputs[1], "deepseek-r1");
    await userEvent.click(screen.getByRole("button", { name: "Save provider" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/providers/manual",
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toMatchObject({
        modelRedirects: [
          { from: "qwen/qwen3.6-35b-a3b", to: "qwen3.6-35b-a3b" },
          { from: "deepseek/deepseek-r1", to: "deepseek-r1" },
        ],
      });
    });
  });

  it("saves full provider model candidates without developer prefix trimming", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];

      if (path === "/api/providers/models") {
        const body = JSON.parse(String(init?.body ?? "{}")) as { providerId?: string };
        return jsonResponse({
          providerId: body.providerId,
          models: body.providerId === "openai" ? ["qwen/qwen3.6-35b-a3b"] : [],
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();
    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Providers" }));
    await userEvent.click(
      await screen.findByRole("button", { name: "Load provider models for OpenAI" }),
    );
    await screen.findByText("qwen/qwen3.6-35b-a3b");

    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Add model" }));
    const modelIdSelect = screen.getByLabelText("Model id");
    await userEvent.click(modelIdSelect);
    expect(
      screen.getByRole("option", { name: "qwen/qwen3.6-35b-a3b" }),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("option", { name: "qwen/qwen3.6-35b-a3b" }));
    changeInput(screen.getByLabelText("Display name"), "Qwen 3.6 35B");
    await userEvent.click(screen.getByRole("button", { name: "Save model" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/models/manual",
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toMatchObject({
        activeProviderId: "openai",
        modelId: "qwen/qwen3.6-35b-a3b",
        providerIds: ["openai"],
      });
    });
  });

  it("reveals a saved provider API key on demand", async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];

      if (path === "/api/providers/reveal-api-key") {
        expect(JSON.parse(String(init?.body ?? "{}"))).toEqual({ id: "openai" });
        return jsonResponse({ apiKey: "sk-saved" });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    await userEvent.click(screen.getByRole("button", { name: "Providers" }));
    await userEvent.click(screen.getByRole("button", { name: "Edit provider OpenAI" }));

    const providerApiKeyInput = screen.getByLabelText("API key");
    expect(providerApiKeyInput).toHaveAttribute("type", "password");
    expect(providerApiKeyInput).toHaveValue("");

    await userEvent.click(screen.getByRole("button", { name: "Show API key" }));

    await waitFor(() => {
      expect(providerApiKeyInput).toHaveAttribute("type", "text");
      expect(providerApiKeyInput).toHaveValue("sk-saved");
    });
    await userEvent.click(screen.getByRole("button", { name: "Save provider" }));
    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/providers/manual",
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(String(saveCall?.[1]?.body))).toMatchObject({
        apiKey: "sk-saved",
        clearApiKey: false,
      });
    });
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
    await expectSelectedOption(screen.getByLabelText("Protocol"), "openai-responses");
    await userEvent.type(screen.getByLabelText("Name"), "Test Provider");
    await userEvent.click(screen.getByRole("checkbox", { name: "Auto sync provider models" }));
    await userEvent.type(screen.getByLabelText("Model sync filter regex"), "^gpt-4");
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
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/providers/manual",
      expect.objectContaining({
        body: expect.stringContaining('"autoSyncModels":true'),
        method: "POST",
      }),
    );
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/providers/manual",
      expect.objectContaining({
        body: expect.stringContaining('"modelSyncFilterRegex":"^gpt-4"'),
        method: "POST",
      }),
    );

    await userEvent.click(screen.getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Add model" }));
    await userEvent.selectOptions(screen.getByLabelText("Model developer"), "openai");
    await userEvent.selectOptions(screen.getByLabelText("Model id"), "created-model");
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
            disabled: ["global:gitmemo"],
            enabled: [],
            translationModelId: null,
          }),
          method: "POST",
        }),
      );
    });
  }, 10000);

  it("saves image-output models without text token limits", async () => {
    const imageAgentDefinition = {
      ...agentDefinitions.agentDefinitions[0],
      allowedExecutionWorkspaceModes: ["shared"],
      allowedTools: ["image_gen", "ask_question", "read_file", "find_files"],
      description: "Built-in agent dedicated to generating images with an image-output model.",
      id: "agent-definition-image-gen",
      maxInstances: 1,
      modelId: "gpt-test",
      name: "Image generation agent",
      permissions: {
        allowedAgentDefinitionIds: [],
        canCreateInstances: false,
        canDelegate: false,
      },
      providerId: "openai",
      systemPrompt: "Use image_gen with model \"gpt-image-2\".",
    };
    let imageModelSaved = false;
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];

      if (path === "/api/models/manual") {
        imageModelSaved = true;
        return mockFetch(input, init);
      }

      if (path === "/api/agent-definitions") {
        return jsonResponse({
          agentDefinitions: imageModelSaved
            ? [...agentDefinitions.agentDefinitions, imageAgentDefinition]
            : agentDefinitions.agentDefinitions,
          defaultRolePrompts: imageModelSaved
            ? { [imageAgentDefinition.id]: imageAgentDefinition.systemPrompt }
            : {},
        });
      }

      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Models" }));
    await userEvent.click(screen.getByRole("button", { name: "Add model" }));
    await userEvent.selectOptions(screen.getByLabelText("Model developer"), "openai");
    await userEvent.selectOptions(screen.getByLabelText("Model id"), "gpt-image-2");
    await userEvent.click(screen.getByRole("button", { name: "Save model" }));

    await waitFor(() => {
      const saveCall = fetchMock.mock.calls.find(
        ([url, init]) =>
          String(url).includes("/api/models/manual") &&
          typeof init?.body === "string" &&
          init.body.includes('"modelId":"gpt-image-2"'),
      );
      expect(saveCall).toBeDefined();
      expect(JSON.parse(saveCall![1]?.body as string)).toMatchObject({
        contextWindow: null,
        maxOutputTokens: null,
        outputModalities: ["image"],
        systemPromptName: "Default",
      });
    });

    await userEvent.click(within(settingsNav).getByRole("button", { name: "Agents" }));
    expect(await screen.findByText("Image generation agent")).toBeInTheDocument();
  });

  it("remote server dialog uses hostname label, root/~, auth tabs, and password payload without echo", async () => {
    const createdServer: RemoteServerSummary = {
      id: "srv-1",
      name: "Lab",
      hostAlias: "10.0.0.8",
      user: "root",
      port: null,
      identityFile: null,
      authMethod: "password",
      passwordConfigured: true,
      defaultRemoteRoot: "~",
      focoCommand: null,
      terminalShell: null,
      connectTimeoutMs: 10000,
      status: "unknown",
      lastError: null,
      lastKnownTarget: null,
      sidecarVersion: null,
      sidecarInstallState: "not_installed",
      workspaceCount: 0,
      lastCheckedAt: null,
    };

    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];
      if (path === "/api/remote-servers/create") {
        const body = JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>;
        expect(body).toMatchObject({
          name: "Lab",
          hostAlias: "10.0.0.8",
          user: "root",
          authMethod: "password",
          password: "s3cret",
          defaultRemoteRoot: "~",
        });
        expect(body).not.toHaveProperty("passwordConfigured");
        appTestState.settingsResponse = {
          ...appTestState.settingsResponse,
          remoteServers: [createdServer],
        };
        return jsonResponse({ server: createdServer });
      }
      if (path === "/api/remote-servers/srv-1/connect") {
        return jsonResponse({
          server: createdServer,
          result: {
            ok: true,
            errorKind: null,
            message: "ok",
            stages: [],
          },
        });
      }
      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Remote Servers" }));

    await userEvent.click(screen.getByRole("button", { name: "Add remote server" }));
    expect(screen.getByLabelText("SSH hostname / IP")).toBeInTheDocument();
    expect(screen.getByLabelText("SSH user")).toHaveValue("root");
    expect(screen.getByLabelText("Default remote root")).toHaveValue("~");
    expect(screen.getByRole("button", { name: "Key" })).toBeEnabled();
    expect(screen.queryByLabelText("SSH password")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Password" }));
    expect(screen.getByRole("button", { name: "Password" })).toBeEnabled();
    expect(screen.queryByPlaceholderText("~/.ssh/id_ed25519")).not.toBeInTheDocument();
    const passwordField = screen.getByLabelText("SSH password");
    expect(passwordField).toHaveAttribute("type", "password");
    expect(passwordField).toHaveValue("");

    await userEvent.type(screen.getByLabelText("Server name"), "Lab");
    await userEvent.type(screen.getByLabelText("SSH hostname / IP"), "10.0.0.8");
    await userEvent.type(passwordField, "s3cret");
    await userEvent.click(screen.getByRole("button", { name: "Save remote server" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(([url]) => url === "/api/remote-servers/create"),
      ).toBe(true);
    });
  });

  it("remote server identity browse and host-key unknown/changed interactions", async () => {
    const existingServer: RemoteServerSummary = {
      id: "srv-key",
      name: "KeyBox",
      hostAlias: "box.example",
      user: "root",
      port: 22,
      identityFile: "/tmp/old-key",
      authMethod: "key",
      passwordConfigured: false,
      defaultRemoteRoot: "~",
      focoCommand: null,
      terminalShell: null,
      connectTimeoutMs: 10000,
      status: "unknown",
      lastError: null,
      lastKnownTarget: null,
      sidecarVersion: null,
      sidecarInstallState: "not_installed",
      workspaceCount: 0,
      lastCheckedAt: null,
    };
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      remoteServers: [existingServer],
    };

    let trustCalled = false;
    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];
      if (path === "/api/remote-servers/srv-key/test") {
        if (trustCalled) {
          return jsonResponse({
            server: existingServer,
            result: { ok: true, errorKind: null, message: "ok", stages: [] },
          });
        }
        return jsonResponse({
          server: existingServer,
          result: {
            ok: false,
            errorKind: "host_key_unknown",
            message: "unknown host key",
            hostKeyVerificationRequired: true,
            hostKey: {
              host: "box.example",
              port: 22,
              algorithm: "ssh-ed25519",
              fingerprintSha256: "SHA256:abcdef",
            },
            stages: [],
          },
        });
      }
      if (path === "/api/remote-servers/srv-key/trust-host-key") {
        trustCalled = true;
        const body = JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>;
        expect(body).toMatchObject({ fingerprintSha256: "SHA256:abcdef" });
        return jsonResponse({ trusted: true, server: existingServer });
      }
      if (path === "/api/file-picker/list") {
        return jsonResponse({
          entries: [
            {
              name: "id_ed25519",
              path: "/home/user/.ssh/id_ed25519",
              isDirectory: false,
              isSymlink: false,
            },
          ],
          path: "/home/user/.ssh",
          parentPath: "/home/user",
        });
      }
      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMock);
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Remote Servers" }));

    await userEvent.click(screen.getByRole("button", { name: "Edit remote server" }));
    expect(screen.getByDisplayValue("/tmp/old-key")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Browse for private key" }));
    // File picker dialog should open for local identity selection and fill back.
    expect(await screen.findByText("Select private key file")).toBeInTheDocument();
    await userEvent.click(await screen.findByRole("button", { name: /id_ed25519/i }));
    await userEvent.click(screen.getByRole("button", { name: "Select" }));
    expect(await screen.findByDisplayValue("/home/user/.ssh/id_ed25519")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Close remote server configuration" }));
    await userEvent.click(screen.getByRole("button", { name: "Test remote server" }));

    expect(await screen.findByRole("dialog", { name: /Unknown SSH host key/i })).toBeInTheDocument();
    expect(screen.getByText("SHA256:abcdef")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Confirm and continue" }));
    await waitFor(() => {
      expect(trustCalled).toBe(true);
      expect(screen.queryByRole("dialog", { name: /Unknown SSH host key/i })).not.toBeInTheDocument();
    });

    // Changed host key shows hard-fail messaging, not trust dialog.
    const fetchMockChanged = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === "string" ? input : input.toString();
      const path = url.split("?")[0];
      if (path === "/api/remote-servers/srv-key/test") {
        return jsonResponse({
          server: existingServer,
          result: {
            ok: false,
            errorKind: "host_key_changed",
            message: "host key changed",
            hostKeyVerificationRequired: false,
            hostKey: {
              host: "box.example",
              port: 22,
              algorithm: "ssh-ed25519",
              fingerprintSha256: "SHA256:newkey",
            },
            stages: [],
          },
        });
      }
      return mockFetch(input, init);
    });
    vi.stubGlobal("fetch", fetchMockChanged);
    await userEvent.click(screen.getByRole("button", { name: "Test remote server" }));
    await waitFor(() => {
      expect(
        screen.getByText(/Host key changed — manual known_hosts fix required/i),
      ).toBeInTheDocument();
    });
    expect(screen.queryByRole("dialog", { name: /Unknown SSH host key/i })).not.toBeInTheDocument();
  });

});
