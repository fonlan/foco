import { describe, expect, it } from "vitest";

import {
  HTML_PREVIEW_IFRAME_SANDBOX,
  isSafeHtmlPreviewUrl,
} from "./WorkspaceHtmlPreviewPanel";

describe("isSafeHtmlPreviewUrl", () => {
  it("accepts preview.localhost hosts", () => {
    expect(isSafeHtmlPreviewUrl("http://abc.preview.localhost:3210/index.html")).toBe(
      true,
    );
    expect(isSafeHtmlPreviewUrl("https://preview.localhost/path")).toBe(true);
  });

  it("rejects foco host and other origins", () => {
    expect(isSafeHtmlPreviewUrl("http://127.0.0.1:3210/api/workspaces")).toBe(false);
    expect(isSafeHtmlPreviewUrl("http://localhost:3210/")).toBe(false);
    expect(isSafeHtmlPreviewUrl("http://evil.preview.localhost.evil.com/")).toBe(
      false,
    );
    expect(isSafeHtmlPreviewUrl("javascript:alert(1)")).toBe(false);
    expect(isSafeHtmlPreviewUrl("not-a-url")).toBe(false);
  });
});

describe("HTML_PREVIEW_IFRAME_SANDBOX", () => {
  it("is limited to scripts and same-origin only", () => {
    expect(HTML_PREVIEW_IFRAME_SANDBOX).toBe("allow-scripts allow-same-origin");
    expect(HTML_PREVIEW_IFRAME_SANDBOX).not.toContain("allow-top-navigation");
    expect(HTML_PREVIEW_IFRAME_SANDBOX).not.toContain("allow-popups");
    expect(HTML_PREVIEW_IFRAME_SANDBOX).not.toContain("allow-downloads");
    expect(HTML_PREVIEW_IFRAME_SANDBOX).not.toContain("allow-forms");
  });
});
