import { describe, expect, it } from "vitest";

import { translate } from "../../shared/i18n";
import { STABLE_REQUEST_KINDS, requestKindLabel } from "./request-kind";

const tEn = (key: string, values?: Record<string, string | number>) =>
  translate(key, values ?? {}, "en");
const tZh = (key: string, values?: Record<string, string | number>) =>
  translate(key, values ?? {}, "zh-CN");

describe("request-kind", () => {
  it("localizes every stable request kind in English and Simplified Chinese", () => {
    const expectedLabels = [
      ["chat completion", "Chat completion", "聊天完成"],
      ["contextCompression", "Context compression", "上下文压缩"],
      ["prompt hook", "Prompt hook", "提示词 Hook"],
      ["chat title generation", "Chat title generation", "会话标题生成"],
      ["memory extraction", "Memory extraction", "记忆抽取"],
      ["memory retrieval", "Memory retrieval", "记忆匹配"],
      ["model availability test", "Model availability test", "模型可用性测试"],
      ["workspace spec generation", "Workspace Spec generation", "Workspace Spec 生成"],
      ["workspace spec update", "Workspace Spec update", "Workspace Spec 更新"],
      ["workspace spec compaction", "Workspace Spec compaction", "Workspace Spec 压缩"],
      [
        "workspace spec update compaction",
        "Workspace Spec update compaction",
        "Workspace Spec 更新压缩",
      ],
      [
        "git_commit_message_generation",
        "Git commit message generation",
        "Git 提交信息生成",
      ],
    ] as const;

    expect(STABLE_REQUEST_KINDS).toEqual(expectedLabels.map(([requestKind]) => requestKind));
    for (const [requestKind, english, chinese] of expectedLabels) {
      expect(requestKindLabel(requestKind, tEn)).toBe(english);
      expect(requestKindLabel(requestKind, tZh)).toBe(chinese);
    }
  });

  it("returns unknown request kinds unchanged", () => {
    expect(requestKindLabel("background maintenance", tEn)).toBe(
      "background maintenance",
    );
    expect(requestKindLabel("background maintenance", tZh)).toBe(
      "background maintenance",
    );
  });
});
