import { describe, expect, it } from "vitest";

import { translate } from "../../shared/i18n";
import { toolDisplayName } from "./chat-helpers";

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
    expect(toolDisplayName("search_query", "zh-CN")).toBe("工具");
    expect(translate("Memories used", {}, "zh-CN")).toBe("使用记忆");
  });
});
