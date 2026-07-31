import { describe, expect, it } from "vitest";

import {
  chatItemClass,
  workspaceItemClass,
  workspaceNewChatButtonClass,
  workspaceNameFromPath,
} from "./workspace-helpers";

describe("workspace helpers", () => {
  it("derives names from Windows and POSIX paths", () => {
    expect(workspaceNameFromPath("C:\\Users\\fonla\\Repos\\Foco\\")).toBe("Foco");
    expect(workspaceNameFromPath("/home/fonla/Foco/")).toBe("Foco");
  });

  it("marks active workspace trigger classes without custom chrome", () => {
    expect(workspaceItemClass(true)).toContain("workspace-item-active");
    expect(workspaceItemClass(true)).not.toContain("rounded-lg");
    expect(workspaceNewChatButtonClass(true)).toContain("workspace-item-active");
    expect(workspaceNewChatButtonClass(true)).toContain("accordion__trigger");
    expect(workspaceNewChatButtonClass(true)).not.toContain("rounded-lg");
  });

  it("keeps chat row helper layout-only", () => {
    expect(workspaceNewChatButtonClass(false)).toContain("workspace-new-chat-button");
    expect(chatItemClass()).toContain("chat-item");
    expect(chatItemClass()).toContain("text-[10px]");
    expect(chatItemClass()).not.toContain("text-xs");
    expect(chatItemClass()).not.toContain("chat-item-active");
  });
});
