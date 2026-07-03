import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  appTestState,
  changeInput,
  jsonResponse,
  mockFetch,
  renderApp,
  resetAppTestEnvironment,
  settings,
} from "./test-utils/app-test-harness";

describe("skill store app surface", () => {
  beforeEach(resetAppTestEnvironment);

  it("opens the skill store from the nav and loads the registry list", async () => {
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
    expect(screen.getAllByText("Total installs").length).toBeGreaterThan(0);
    expect(screen.queryByText("Hot skills in the last 24h")).not.toBeInTheDocument();
    expect(
      fetchMock.mock.calls.some(
        ([url]) => url === "/api/skill-store/browse?sort=installs_desc",
      ),
    ).toBe(true);
    expect(
      fetchMock.mock.calls.some(([url]) =>
        String(url).startsWith("/api/skill-store/skills/browser-scout?"),
      ),
    ).toBe(true);
  });

  it("reloads the registry list when the sort changes", async () => {
    const fetchMock = vi.mocked(fetch);
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Skill Store" }));
    expect(await screen.findAllByText("Browser Scout")).not.toHaveLength(0);

    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Sort skills" }),
      "name_asc",
    );

    await waitFor(() =>
      expect(
        fetchMock.mock.calls.some(
          ([url]) => url === "/api/skill-store/browse?sort=name_asc",
        ),
      ).toBe(true),
    );
    expect(screen.getAllByText("Name A-Z").length).toBeGreaterThan(0);
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
    const scriptsItem = within(detailPane).getByText("scripts").closest("li");
    expect(scriptsItem).not.toBeNull();
    expect(within(scriptsItem as HTMLElement).getByText("search.md")).toBeInTheDocument();
    expect(within(detailPane).getByTitle("scripts/search.md")).toBeInTheDocument();
    expect(within(detailPane).queryByText("scripts/search.md")).not.toBeInTheDocument();
    expect(
      within(detailPane).getByRole("heading", { level: 1, name: "Browser Scout" }),
    ).toBeInTheDocument();

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

  it("hides the summary translation button until a model is configured", async () => {
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Skill Store" }));
    const detailPane = await screen.findByRole("region", { name: "Skill details" });
    expect(await within(detailPane).findByText("Use this skill to collect focused web references.")).toBeInTheDocument();

    expect(within(detailPane).queryByRole("button", { name: "Translate" })).not.toBeInTheDocument();
  });

  it("translates the summary once and toggles back to the original", async () => {
    const fetchMock = vi.mocked(fetch);
    const skillContent = [
      "---",
      "name: browser-scout",
      "description: Find useful web references.",
      "---",
      "",
      "# Browser Scout",
      "",
      "Use this skill to collect focused web references.",
    ].join("\n");
    appTestState.settingsResponse = {
      ...settings,
      skills: { ...settings.skills, translationModelId: "gpt-test" },
    };
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Skill Store" }));
    const detailPane = await screen.findByRole("region", { name: "Skill details" });
    expect(await within(detailPane).findByText("Use this skill to collect focused web references.")).toBeInTheDocument();

    await userEvent.click(within(detailPane).getByRole("button", { name: "Translate" }));

    await waitFor(() =>
      expect(within(detailPane).getByText("Translated SKILL.md summary")).toBeInTheDocument(),
    );
    const translateCalls = fetchMock.mock.calls.filter(
      ([url, init]) => url === "/api/skill-store/translate" && init?.method === "POST",
    );
    expect(translateCalls).toHaveLength(1);
    expect(JSON.parse(String(translateCalls[0]?.[1]?.body))).toEqual({
      content: skillContent,
      targetLanguage: "en",
    });

    await userEvent.click(within(detailPane).getByRole("button", { name: "Show original" }));

    expect(within(detailPane).getByText("Use this skill to collect focused web references.")).toBeInTheDocument();
    expect(
      fetchMock.mock.calls.filter(
        ([url, init]) => url === "/api/skill-store/translate" && init?.method === "POST",
      ),
    ).toHaveLength(1);
  });

  it("shows localized full SKILL.md summary instead of README", async () => {
    const longSkillSummary = [
      "---",
      "name: browser-scout",
      "description: Find useful web references.",
      "---",
      "",
      "## Markdown Check",
      "",
      "- **bold marker**",
      "",
      "SKILL.md summary starts here.",
      "x".repeat(1500),
      "SKILL_MD_TAIL_SENTINEL",
    ].join("\n");
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

        if (path === "/api/settings") {
          return jsonResponse(zhSettings);
        }
        if (path === "/api/skill-store/skills/browser-scout") {
          return jsonResponse({
            description: "Find useful web references.",
            files: [
              { path: "SKILL.md", content: longSkillSummary },
              { path: "README.md", content: "README-only summary text" },
            ],
            id: "browser-scout",
            name: "Browser Scout",
            source: "foco/browser-scout",
          });
        }

        return mockFetch(input, init);
      }),
    );
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "技能商店" }));
    const detailPane = await screen.findByRole("region", { name: "技能详情" });

    expect(await within(detailPane).findByRole("heading", { name: "摘要" })).toBeInTheDocument();
    expect(within(detailPane).getByRole("button", { name: "安装" })).toBeInTheDocument();
    expect(
      within(detailPane).getByRole("heading", { level: 2, name: "Markdown Check" }),
    ).toBeInTheDocument();
    expect(within(detailPane).getByText("bold marker").tagName).toBe("STRONG");
    expect(within(detailPane).getByText(/SKILL_MD_TAIL_SENTINEL/)).toBeInTheDocument();
    expect(within(detailPane).queryByText("README-only summary text")).not.toBeInTheDocument();
  });
});
