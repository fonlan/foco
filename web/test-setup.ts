import "@testing-library/jest-dom/vitest";
import { afterEach, vi } from "vitest";

class MockResizeObserver implements ResizeObserver {
  constructor(private readonly callback: ResizeObserverCallback) {}

  observe(target: Element) {
    const contentRect = {
      bottom: 300,
      height: 300,
      left: 0,
      right: 800,
      toJSON: () => ({}),
      top: 0,
      width: 800,
      x: 0,
      y: 0,
    } satisfies DOMRectReadOnly;

    this.callback(
      [
        {
          borderBoxSize: [],
          contentBoxSize: [],
          contentRect,
          devicePixelContentBoxSize: [],
          target,
        } satisfies ResizeObserverEntry,
      ],
      this,
    );
  }

  unobserve() {}
  disconnect() {}
}

class MockWebSocket extends EventTarget implements WebSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;

  readonly CONNECTING = 0;
  readonly OPEN = 1;
  readonly CLOSING = 2;
  readonly CLOSED = 3;
  binaryType: BinaryType = "blob";
  bufferedAmount = 0;
  extensions = "";
  onclose: ((this: WebSocket, ev: CloseEvent) => unknown) | null = null;
  onerror: ((this: WebSocket, ev: Event) => unknown) | null = null;
  onmessage: ((this: WebSocket, ev: MessageEvent) => unknown) | null = null;
  onopen: ((this: WebSocket, ev: Event) => unknown) | null = null;
  protocol = "";
  readyState: 0 | 1 | 2 | 3 = MockWebSocket.CONNECTING;
  url: string;

  constructor(url: string | URL) {
    super();
    this.url = String(url);
    queueMicrotask(() => {
      this.readyState = MockWebSocket.OPEN;
      const event = new Event("open");
      this.onopen?.call(this, event);
      this.dispatchEvent(event);
    });
  }

  close() {
    this.readyState = MockWebSocket.CLOSED;
    const event = new CloseEvent("close");
    this.onclose?.call(this, event);
    this.dispatchEvent(event);
  }

  send() {}
}

class MockClipboardItem implements ClipboardItem {
  readonly presentationStyle = "unspecified";
  readonly types: string[];

  constructor(items: Record<string, unknown>) {
    this.types = Object.keys(items);
    for (const item of Object.values(items)) {
      if (item && typeof (item as Promise<unknown>).then === "function") {
        void (item as Promise<unknown>).catch(() => undefined);
      }
    }
  }

  async getType(_type: string) {
    return new Blob();
  }
}

class MockWorker extends EventTarget {
  onerror: ((this: Worker, ev: ErrorEvent) => unknown) | null = null;
  onmessage: ((this: Worker, ev: MessageEvent) => unknown) | null = null;
  onmessageerror: ((this: Worker, ev: MessageEvent) => unknown) | null = null;

  constructor(_scriptURL: string | URL) {
    super();
  }

  postMessage() {}
  terminate() {}
}

Object.defineProperty(window.HTMLElement.prototype, "scrollIntoView", {
  configurable: true,
  value: vi.fn(),
});

Object.defineProperty(window.HTMLElement.prototype, "setPointerCapture", {
  configurable: true,
  value: vi.fn(),
});

Object.defineProperty(window.HTMLElement.prototype, "releasePointerCapture", {
  configurable: true,
  value: vi.fn(),
});

Object.defineProperty(window.HTMLCanvasElement.prototype, "getContext", {
  configurable: true,
  value: vi.fn(() => null),
});

Object.defineProperty(window, "ResizeObserver", {
  configurable: true,
  value: MockResizeObserver,
});

Object.defineProperty(window, "WebSocket", {
  configurable: true,
  value: MockWebSocket,
});

Object.defineProperty(window, "ClipboardItem", {
  configurable: true,
  value: MockClipboardItem,
});

// ponytail: enough for Monaco diagnostics startup; upgrade when tests assert worker messaging.
Object.defineProperty(window, "Worker", {
  configurable: true,
  value: MockWorker,
});

Object.defineProperty(globalThis, "Worker", {
  configurable: true,
  value: MockWorker,
});

Object.defineProperty(globalThis, "ClipboardItem", {
  configurable: true,
  value: MockClipboardItem,
});

Object.defineProperty(window, "requestAnimationFrame", {
  configurable: true,
  value: (callback: FrameRequestCallback) => window.setTimeout(callback, 0),
});

Object.defineProperty(window, "matchMedia", {
  configurable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    addEventListener: vi.fn(),
    addListener: vi.fn(),
    dispatchEvent: vi.fn(),
    matches: false,
    media: query,
    onchange: null,
    removeEventListener: vi.fn(),
    removeListener: vi.fn(),
  })),
});

Object.defineProperty(navigator, "clipboard", {
  configurable: true,
  value: {
    write: vi.fn().mockResolvedValue(undefined),
    writeText: vi.fn().mockResolvedValue(undefined),
  },
});

afterEach(() => {
  vi.restoreAllMocks();
});
