import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  changeInput,
  renderApp,
  resetAppTestEnvironment,
} from "./test-utils/app-test-harness";

describe("skill store app surface", () => {
  beforeEach(resetAppTestEnvironment);

  it("opens the skill store from the nav and loads the hot list", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    const storeButton = await screen.findByRole("button", { name: "Skill Store" });
    await userEvent.click(storeButton);

    expect(await screen.findByRole("heading", { name: "Skill Store" })).toBeInTheDocument();
    expect(window.location.pathname).toBe("/skills");
    expect(screen.getByRole("button", { name: "Skill Store" })).toHaveClass(
      "foco-nav-rail-button-active",
    );
    expect(await screen.findByText("Browser Scout")).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.some(([url]) => url === "/api/skill-store/hot"),
    ).toBe(true);
    expect(
      fetchMock.mock.calls.some(([url]) =>
        String(url).startsWith("/api/skill-store/skills/browser-scout?"),
      ),
    ).toBe(true);
  });

  it("searches and installs a skill through the backend proxy", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Skill Store" }));
    expect(await screen.findAllByText("Browser Scout")).not.toHaveLength(0);

    changeInput(screen.getByRole("searchbox", { name: "Search skills" }), "browser");

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(([url]) =>
          String(url).includes("/api/skill-store/search?query=browser"),
        ),
      ).toBe(true),
    );

    const detailPane = await screen.findByRole("region", { name: "Skill details" });
    expect(within(detailPane).getByText("scripts/search.md")).toBeInTheDocument();

    await userEvent.click(within(detailPane).getByRole("button", { name: "Install" }));

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(
          ([url, init]) => url === "/api/skill-store/install" && init?.method === "POST",
        ),
      ).toBe(true),
    );
    expect(
      fetchMock.mock.calls.some(
        ([url, init]) => url === "/api/skills/refresh" && init?.method === "POST",
      ),
    ).toBe(true);
    expect(await screen.findByText(/Installed skill to/)).toBeInTheDocument();
  });
});
