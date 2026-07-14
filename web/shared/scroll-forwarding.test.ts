import { afterEach, describe, expect, it } from "vitest";

import {
  findVerticalScrollAncestor,
  forwardWheelAtVerticalBoundary,
  wheelDeltaPixels,
} from "./scroll-forwarding";

function mockScrollMetrics(
  element: HTMLElement,
  metrics: { clientHeight: number; scrollHeight: number; scrollTop?: number },
) {
  Object.defineProperties(element, {
    clientHeight: { configurable: true, value: metrics.clientHeight },
    scrollHeight: { configurable: true, value: metrics.scrollHeight },
  });
  element.scrollTop = metrics.scrollTop ?? 0;
  return element;
}

describe("scroll-forwarding", () => {
  afterEach(() => {
    document.body.replaceChildren();
  });

  it("finds the nearest vertical overflow ancestor that can scroll", () => {
    const outer = document.createElement("div");
    outer.style.overflowY = "auto";
    const middle = document.createElement("div");
    middle.style.overflowY = "visible";
    const inner = document.createElement("div");
    outer.append(middle);
    middle.append(inner);
    document.body.append(outer);

    mockScrollMetrics(outer, { clientHeight: 100, scrollHeight: 400 });
    mockScrollMetrics(middle, { clientHeight: 200, scrollHeight: 200 });

    expect(findVerticalScrollAncestor(inner)).toBe(outer);
    expect(findVerticalScrollAncestor(middle)).toBe(outer);
  });

  it("converts wheel deltaMode to pixels", () => {
    expect(
      wheelDeltaPixels(
        { deltaMode: WheelEvent.DOM_DELTA_PIXEL, deltaY: 40 },
        300,
      ),
    ).toBe(40);
    expect(
      wheelDeltaPixels(
        { deltaMode: WheelEvent.DOM_DELTA_LINE, deltaY: 3 },
        300,
      ),
    ).toBe(48);
    expect(
      wheelDeltaPixels(
        { deltaMode: WheelEvent.DOM_DELTA_PAGE, deltaY: 1 },
        300,
      ),
    ).toBe(300);
  });

  it("forwards vertical wheel only at boundaries and preventDefaults when outer moves", () => {
    const outer = document.createElement("div");
    outer.style.overflowY = "auto";
    const inner = document.createElement("div");
    outer.append(inner);
    document.body.append(outer);

    mockScrollMetrics(outer, {
      clientHeight: 300,
      scrollHeight: 900,
      scrollTop: 200,
    });
    mockScrollMetrics(inner, {
      clientHeight: 120,
      scrollHeight: 400,
      scrollTop: 100,
    });

    const midWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: 40,
    });
    Object.defineProperty(midWheel, "currentTarget", {
      configurable: true,
      value: inner,
    });
    expect(forwardWheelAtVerticalBoundary(midWheel, inner)).toBe(false);
    expect(outer.scrollTop).toBe(200);
    expect(midWheel.defaultPrevented).toBe(false);

    inner.scrollTop = 280;
    const bottomWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: 40,
    });
    expect(forwardWheelAtVerticalBoundary(bottomWheel, inner)).toBe(true);
    expect(outer.scrollTop).toBe(240);
    expect(bottomWheel.defaultPrevented).toBe(true);

    inner.scrollTop = 0;
    const topWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: -30,
    });
    expect(forwardWheelAtVerticalBoundary(topWheel, inner)).toBe(true);
    expect(outer.scrollTop).toBe(210);
    expect(topWheel.defaultPrevented).toBe(true);

    inner.scrollTop = 280;
    const horizontalWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaX: 50,
      deltaY: 10,
    });
    expect(forwardWheelAtVerticalBoundary(horizontalWheel, inner)).toBe(false);
    expect(outer.scrollTop).toBe(210);
    expect(horizontalWheel.defaultPrevented).toBe(false);
  });

  it("uses line deltaMode when forwarding to the outer scroller", () => {
    const outer = document.createElement("div");
    outer.style.overflowY = "auto";
    const inner = document.createElement("div");
    outer.append(inner);
    document.body.append(outer);

    mockScrollMetrics(outer, {
      clientHeight: 300,
      scrollHeight: 900,
      scrollTop: 10,
    });
    mockScrollMetrics(inner, {
      clientHeight: 120,
      scrollHeight: 120,
      scrollTop: 0,
    });

    const lineWheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaMode: WheelEvent.DOM_DELTA_LINE,
      deltaY: 3,
    });
    expect(forwardWheelAtVerticalBoundary(lineWheel, inner)).toBe(true);
    expect(outer.scrollTop).toBe(58);
    expect(lineWheel.defaultPrevented).toBe(true);
  });

  it("does not preventDefault when the outer scroller cannot move further", () => {
    const outer = document.createElement("div");
    outer.style.overflowY = "auto";
    const inner = document.createElement("div");
    outer.append(inner);
    document.body.append(outer);

    mockScrollMetrics(outer, {
      clientHeight: 300,
      scrollHeight: 300,
      scrollTop: 0,
    });
    mockScrollMetrics(inner, {
      clientHeight: 120,
      scrollHeight: 400,
      scrollTop: 0,
    });

    // Make findVerticalScrollAncestor skip outer because it cannot scroll.
    const wheel = new WheelEvent("wheel", {
      bubbles: true,
      cancelable: true,
      deltaY: -20,
    });
    expect(forwardWheelAtVerticalBoundary(wheel, inner)).toBe(false);
    expect(wheel.defaultPrevented).toBe(false);
  });
});
