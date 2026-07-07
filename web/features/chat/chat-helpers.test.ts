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
    expect(translate("Memories used", {}, "zh-CN")).toBe("使用记忆");
  });
});
