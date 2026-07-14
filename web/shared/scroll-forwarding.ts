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

export function wheelDeltaPixels(
  event: Pick<WheelEvent, "deltaMode" | "deltaY">,
  pageHeight: number,
): number {
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) {
    return event.deltaY * 16;
  }
  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) {
    return event.deltaY * pageHeight;
  }
  return event.deltaY;
}

/**
 * When an inner scroller is already at a vertical edge, forward the wheel
 * delta to the nearest outer vertical scroll ancestor and preventDefault only
 * if the outer scroller actually moved.
 * Returns true when the outer scroller moved.
 */
export function forwardWheelAtVerticalBoundary(
  event: WheelEvent,
  scroller: HTMLElement,
): boolean {
  if (
    event.deltaY === 0 ||
    Math.abs(event.deltaY) <= Math.abs(event.deltaX)
  ) {
    return false;
  }

  const atTop = scroller.scrollTop <= 0;
  const atBottom =
    scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 1;
  if ((event.deltaY < 0 && !atTop) || (event.deltaY > 0 && !atBottom)) {
    return false;
  }

  const scrollAncestor = findVerticalScrollAncestor(scroller.parentElement);
  if (!scrollAncestor || scrollAncestor === scroller) {
    return false;
  }

  const previousScrollTop = scrollAncestor.scrollTop;
  scrollAncestor.scrollTop += wheelDeltaPixels(
    event,
    scrollAncestor.clientHeight,
  );
  if (scrollAncestor.scrollTop !== previousScrollTop) {
    event.preventDefault();
    return true;
  }
  return false;
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
