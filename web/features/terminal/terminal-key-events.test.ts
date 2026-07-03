import { describe, expect, it } from "vitest";

import { fallbackTerminalInputForKeyEvent } from "./terminal-key-events";

function keyEvent(overrides: Partial<KeyboardEvent> = {}) {
  return {
    altKey: false,
    ctrlKey: false,
    key: "Backspace",
    metaKey: false,
    type: "keydown",
    ...overrides,
  } as Pick<KeyboardEvent, "altKey" | "ctrlKey" | "key" | "metaKey" | "type">;
}

describe("terminal key events", () => {
  it("falls back to DEL for plain Backspace", () => {
    expect(fallbackTerminalInputForKeyEvent(keyEvent())).toBe("\x7f");
  });

  it("leaves modified Backspace to xterm", () => {
    expect(fallbackTerminalInputForKeyEvent(keyEvent({ altKey: true }))).toBeNull();
    expect(fallbackTerminalInputForKeyEvent(keyEvent({ ctrlKey: true }))).toBeNull();
    expect(fallbackTerminalInputForKeyEvent(keyEvent({ metaKey: true }))).toBeNull();
  });
});
