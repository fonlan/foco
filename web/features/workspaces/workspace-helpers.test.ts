import { describe, expect, it } from "vitest";

import { chatItemClass, workspaceItemClass, workspaceMenuClass, workspaceNameFromPath } from "./workspace-helpers";

describe("workspace helpers", () => {
  it("derives names from Windows and POSIX paths", () => {
    expect(workspaceNameFromPath("C:\\Users\\fonla\\Repos\\Foco\\")).toBe("Foco");
    expect(workspaceNameFromPath("/home/fonla/Foco/")).toBe("Foco");
  });

  it("marks active classes explicitly", () => {
    expect(workspaceItemClass(true)).toContain("workspace-item-active");
    expect(workspaceMenuClass(true)).toContain("workspace-menu-active");
    expect(chatItemClass(true)).toContain("chat-item-active");
    expect(chatItemClass(false)).not.toContain("chat-item-active");
  });
});
