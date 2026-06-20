import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  defaultComposerPlaceholder,
  jsonResponse,
  mockFetch,
  renderApp,
  resetAppTestEnvironment,
  settings,
} from "./test-utils/app-test-harness";

describe("app agents verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  it("shows Agent definitions in settings", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Agents" }));

    expect(await screen.findByText("Agent definitions")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Agent settings" })).toBeInTheDocument();
    expect(screen.getAllByText("Coordinator").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Worker").length).toBeGreaterThan(0);
    expect(screen.getByText("Coordinates the Agent team.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Edit agent Coordinator" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete agent Coordinator" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Add agent definition" }));
    const dialog = screen.getByRole("dialog", { name: "Create agent" });
    expect(within(dialog).getByLabelText("System prompt")).toHaveValue("Default");
    await userEvent.click(within(dialog).getByText("Allowed tools"));
    await userEvent.click(within(dialog).getByRole("checkbox", { name: "read_file" }));
    expect(within(dialog).getByText("1 selected")).toBeInTheDocument();
  });

  it("localizes the Agents settings surface", async () => {
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
    await userEvent.click(within(settingsNav).getByRole("button", { name: "智能体" }));

    expect(await screen.findByRole("heading", { name: "智能体设置" })).toBeInTheDocument();
    expect(screen.getByText("智能体定义、模型、工具与权限")).toBeInTheDocument();
    expect(screen.queryByText("技能设置")).not.toBeInTheDocument();
  });

  it("opens the Agents panel and enables a Team", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByText("Tool run"));
    await userEvent.click(await screen.findByRole("button", { name: "Agents" }));

    expect(await screen.findByText("Team is not enabled")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Enable" }));

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/chats/chat-1/agent-team/enable",
        expect.objectContaining({
          body: expect.stringContaining("agent-definition-coordinator"),
          method: "POST",
        }),
      );
    });
    expect(await screen.findByText("agent-team-1")).toBeInTheDocument();
    expect(screen.getByText("team_created")).toBeInTheDocument();
    expect(screen.getByText("Observability")).toBeInTheDocument();
    expect(screen.getByText("Queue wait")).toBeInTheDocument();
    expect(screen.getByText("Worker, inspect the current task.")).toBeInTheDocument();
  });

  it("queues the first message with Team tools disabled by default from the composer", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const teamToggle = await screen.findByRole("button", { name: "Team mode" });
    expect(teamToggle).toHaveAttribute("aria-pressed", "false");

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "handle this",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      const [, init] = queueCall!;
      expect(JSON.parse(init?.body as string)).toMatchObject({
        message: "handle this",
        teamModeEnabled: false,
      });
    });
  });

  it("queues the first message with Team tools enabled from the composer", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const teamToggle = await screen.findByRole("button", { name: "Team mode" });
    expect(teamToggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(teamToggle);
    expect(teamToggle).toHaveAttribute("aria-pressed", "true");

    await userEvent.type(
      await screen.findByPlaceholderText(defaultComposerPlaceholder),
      "coordinate this",
    );
    await userEvent.click(screen.getByRole("button", { name: "Send message" }));

    await waitFor(() => {
      const queueCall = fetchMock.mock.calls.find(
        ([url]) => url === "/api/workspaces/workspace-1/chat/queue",
      );
      expect(queueCall).toBeDefined();
      const [, init] = queueCall!;
      expect(JSON.parse(init?.body as string)).toMatchObject({
        message: "coordinate this",
        teamModeEnabled: true,
      });
    });
  });
});
