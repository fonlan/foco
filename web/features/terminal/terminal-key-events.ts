export type TerminalKeyEvent = Pick<
  KeyboardEvent,
  "altKey" | "ctrlKey" | "key" | "metaKey" | "type"
>;

export function fallbackTerminalInputForKeyEvent(event: TerminalKeyEvent) {
  if (
    event.type === "keydown" &&
    event.key === "Backspace" &&
    !event.altKey &&
    !event.ctrlKey &&
    !event.metaKey
  ) {
    return "\x7f";
  }

  return null;
}
