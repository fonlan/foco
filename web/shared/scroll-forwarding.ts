export type VerticalTouchDragState = {
  lastY: number;
  startX: number;
  startY: number;
};

export function findVerticalScrollAncestor(node: HTMLElement | null) {
  while (node) {
    const overflowY = window.getComputedStyle(node).overflowY;
    if (
      /(auto|scroll)/.test(overflowY) &&
      node.scrollHeight > node.clientHeight
    ) {
      return node;
    }
    node = node.parentElement;
  }
  return null;
}

export function startVerticalTouchDragForward(
  touch: Pick<Touch, "clientX" | "clientY">,
): VerticalTouchDragState {
  return {
    lastY: touch.clientY,
    startX: touch.clientX,
    startY: touch.clientY,
  };
}

export function forwardVerticalTouchDrag(
  state: VerticalTouchDragState | null,
  touch: Pick<Touch, "clientX" | "clientY">,
  currentTarget: HTMLElement,
) {
  if (!state) {
    return false;
  }

  const deltaX = Math.abs(touch.clientX - state.startX);
  const deltaY = Math.abs(touch.clientY - state.startY);
  if (deltaX >= deltaY) {
    state.lastY = touch.clientY;
    return false;
  }

  const scrollDelta = state.lastY - touch.clientY;
  state.lastY = touch.clientY;
  const scrollAncestor = findVerticalScrollAncestor(currentTarget.parentElement);
  if (!scrollAncestor) {
    return false;
  }

  scrollAncestor.scrollTop += scrollDelta;
  return true;
}
