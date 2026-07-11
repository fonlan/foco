import { describe, expect, it } from "vitest";

import { translate } from "../../shared/i18n";
import { messageWithSelectedSkills, toolDisplayName } from "./chat-helpers";

describe("chat display helpers", () => {
  it("localizes known tool names and memory labels", () => {
    expect(toolDisplayName("read_file", "en")).toBe("Read");
    expect(toolDisplayName("run_command", "zh-CN")).toBe("运行");
    expect(toolDisplayName("agent_list", "en")).toBe("List Agents");
    expect(toolDisplayName("agent_list", "zh-CN")).toBe("列出智能体");
    expect(toolDisplayName("update_todo_graph", "zh-CN")).toBe("更新待办");
    expect(toolDisplayName("update_todo_graph", "en")).toBe("Update Todos");
    expect(toolDisplayName("web_search", "en")).toBe("Web Search");
    expect(toolDisplayName("web_search", "zh-CN")).toBe("Web搜索");
    expect(toolDisplayName("get_plans", "en")).toBe("Get Plans");
    expect(toolDisplayName("get_plans", "zh-CN")).toBe("获取计划");
    expect(toolDisplayName("graph_explore", "zh-CN")).toBe("代码探索");
    expect(toolDisplayName("memory_search", "zh-CN")).toBe("搜索记忆");
    expect(toolDisplayName("read_spec", "zh-CN")).toBe("读取 Spec");
    expect(toolDisplayName("mcp__context7__query-docs", "en")).toBe(
      "Query Documentation",
    );
    expect(toolDisplayName("mcp__context7__query-docs", "zh-CN")).toBe("查询文档");
    expect(toolDisplayName("mcp__custom__unmapped-tool", "en")).toBe(
      "mcp__custom__unmapped-tool",
    );
    expect(toolDisplayName("mcp__custom__unmapped-tool", "zh-CN")).toBe(
      "mcp__custom__unmapped-tool",
    );
    expect(toolDisplayName("search_query", "en")).toBe("Search Query");
    expect(toolDisplayName("search_query", "zh-CN")).toBe("工具");
    expect(translate("Memories used", {}, "zh-CN")).toBe("使用记忆");
  });

  it("treats invalid selected skill ids as no skills", () => {
    const skills = [
      {
        canEnable: true,
        description: "Project memory.",
        enabled: true,
        id: "gitmemo",
        key: "global:gitmemo",
        name: "gitmemo",
        path: "/skills/gitmemo/SKILL.md",
        scope: "global" as const,
        warnings: [],
        workspaceId: null,
        workspaceName: null,
      },
    ];

    expect(messageWithSelectedSkills(skills, null, "hello")).toBe("hello");
    expect(messageWithSelectedSkills(skills, undefined, "hello")).toBe("hello");
    expect(messageWithSelectedSkills(skills, ["unknown"], "hello")).toBe("hello");
    expect(messageWithSelectedSkills(skills, ["global:gitmemo"], "hello")).toBe(
      "[$gitmemo](/skills/gitmemo/SKILL.md) hello",
    );
  });
});
