import { describe, expect, it } from "vitest";

import { translate } from "../../shared/i18n";
import {
  formatProviderTransportSuffix,
  normalizeRequestTransport,
  requestTransportLabel,
} from "./request-transport";

const tEn = (key: string, values?: Record<string, string | number>) =>
  translate(key, values ?? {}, "en");
const tZh = (key: string, values?: Record<string, string | number>) =>
  translate(key, values ?? {}, "zh-CN");

describe("request-transport", () => {
  it("normalizes only the stable wire-derived transport union", () => {
    expect(normalizeRequestTransport("http")).toBe("http");
    expect(normalizeRequestTransport("websocket")).toBe("websocket");
    expect(normalizeRequestTransport("unknown")).toBe("unknown");
    expect(normalizeRequestTransport("stdio")).toBe("unknown");
    expect(normalizeRequestTransport(null)).toBe("unknown");
    expect(normalizeRequestTransport(undefined)).toBe("unknown");
  });

  it("labels HTTP, WebSocket, and unknown for list and detail", () => {
    expect(requestTransportLabel("http", tEn)).toBe("HTTP");
    expect(requestTransportLabel("websocket", tEn)).toBe("WebSocket");
    expect(requestTransportLabel("unknown", tEn)).toBe("Unknown");
    expect(requestTransportLabel("http", tZh)).toBe("HTTP");
    expect(requestTransportLabel("websocket", tZh)).toBe("WebSocket");
    expect(requestTransportLabel("unknown", tZh)).toBe("未知");
  });

  it("formats provider-row parentheses without changing the model line", () => {
    expect(formatProviderTransportSuffix("http", tEn)).toBe("(HTTP)");
    expect(formatProviderTransportSuffix("websocket", tEn)).toBe("(WebSocket)");
    expect(formatProviderTransportSuffix("unknown", tEn)).toBe("(Unknown)");
    expect(formatProviderTransportSuffix("http", tZh)).toBe("（HTTP）");
    expect(formatProviderTransportSuffix("websocket", tZh)).toBe("（WebSocket）");
    expect(formatProviderTransportSuffix("unknown", tZh)).toBe("（未知）");
  });
});
