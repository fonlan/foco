import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  renderApp,
  resetAppTestEnvironment,
} from "./test-utils/app-test-harness";

describe("app agents verification surfaces", () => {
  beforeEach(resetAppTestEnvironment);

  it("shows Agent definitions in settings", async () => {
    renderApp();

    await userEvent.click((await screen.findAllByRole("button", { name: "Settings" }))[0]);
    const settingsNav = await screen.findByRole("navigation", { name: "Settings" });
    await userEvent.click(within(settingsNav).getByRole("button", { name: "Agents" }));

    expect(await screen.findByText("Agent definitions")).toBeInTheDocument();
    expect(screen.getAllByText("Coordinator").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Worker").length).toBeGreaterThan(0);
    expect(screen.getByText("Create agent")).toBeInTheDocument();
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
});
