import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  HTML_PREVIEW_IFRAME_SANDBOX,
  WorkspaceHtmlPreviewPanel,
  isSafeHtmlPreviewUrl,
} from "./WorkspaceHtmlPreviewPanel";

const requestJsonMock = vi.fn();

vi.mock("../../shared/api-client", () => ({
  errorMessage: (error: unknown) =>
    error instanceof Error ? error.message : String(error ?? ""),
  requestJson: (...args: unknown[]) => requestJsonMock(...args),
}));

vi.mock("../../shared/i18n", () => ({
  useI18n: () => ({
    t: (key: string, values?: Record<string, string | number>) => {
      if (!values) {
        return key;
      }
      return Object.entries(values).reduce(
        (text, [name, value]) => text.replace(`{${name}}`, String(value)),
        key,
      );
    },
  }),
}));

const tab = {
  name: "index.html",
  path: "demo/index.html",
  workspaceId: "workspace-1",
  workspaceLogoUrl: null,
  workspaceName: "Default",
};

function previewSession(token = "abcdefghijklmnopqrstuvwxyz012345") {
  return {
    entryPath: "demo/index.html",
    iframeSandbox: "allow-scripts allow-same-origin allow-forms",
    previewOrigin: `http://${token}.preview.localhost:3210`,
    previewUrl: `http://${token}.preview.localhost:3210/index.html`,
    rootPath: "demo",
    token,
    workspaceId: "workspace-1",
  };
}

function mockPreviewApi(options?: {
  create?: (path: string, attempt: number) => unknown | Promise<unknown>;
}) {
  let createAttempt = 0;
  requestJsonMock.mockImplementation(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    if (method === "DELETE" && String(url).includes("/preview/sessions/")) {
      return { released: true };
    }
    if (method === "POST" && String(url).endsWith("/preview/sessions")) {
      createAttempt += 1;
      const body =
        typeof init?.body === "string"
          ? (JSON.parse(init.body) as { path?: string })
          : {};
      if (options?.create) {
        return options.create(body.path ?? "", createAttempt);
      }
      return previewSession(
        `token${String(createAttempt).padStart(27, "0")}`.slice(0, 32),
      );
    }
    throw new Error(`unexpected request: ${method} ${url}`);
  });
}

describe("isSafeHtmlPreviewUrl", () => {
  it("accepts token.preview.localhost hosts", () => {
    expect(isSafeHtmlPreviewUrl("http://abc.preview.localhost:3210/index.html")).toBe(
      true,
    );
    expect(
      isSafeHtmlPreviewUrl(
        "https://abcdefghijklmnopqrstuvwxyz012345.preview.localhost/path",
      ),
    ).toBe(true);
  });

  it("rejects foco host, bare preview host, and other origins", () => {
    expect(isSafeHtmlPreviewUrl("http://127.0.0.1:3210/api/workspaces")).toBe(false);
    expect(isSafeHtmlPreviewUrl("http://localhost:3210/")).toBe(false);
    expect(isSafeHtmlPreviewUrl("https://preview.localhost/path")).toBe(false);
    expect(isSafeHtmlPreviewUrl("http://evil.preview.localhost.evil.com/")).toBe(
      false,
    );
    expect(isSafeHtmlPreviewUrl("http://nested.token.preview.localhost/")).toBe(
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

describe("WorkspaceHtmlPreviewPanel", () => {
  beforeEach(() => {
    requestJsonMock.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("creates a session and renders a sandboxed iframe", async () => {
    mockPreviewApi({
      create: () => previewSession(),
    });

    render(<WorkspaceHtmlPreviewPanel tab={tab} />);

    expect(await screen.findByTitle("HTML preview for index.html")).toBeInTheDocument();
    const iframe = screen.getByTitle("HTML preview for index.html");
    expect(iframe).toHaveAttribute(
      "src",
      "http://abcdefghijklmnopqrstuvwxyz012345.preview.localhost:3210/index.html",
    );
    expect(iframe).toHaveAttribute("sandbox", HTML_PREVIEW_IFRAME_SANDBOX);
    expect(iframe).toHaveAttribute("referrerpolicy", "no-referrer");

    expect(requestJsonMock).toHaveBeenCalledWith(
      "/api/workspaces/workspace-1/preview/sessions",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ path: "demo/index.html" }),
      }),
    );
  });

  it("rejects unsafe preview URLs from the API", async () => {
    mockPreviewApi({
      create: () => ({
        ...previewSession(),
        previewUrl: "http://127.0.0.1:3210/evil",
      }),
    });

    render(<WorkspaceHtmlPreviewPanel tab={tab} />);

    expect(
      await screen.findByText(
        "HTML preview returned an unsafe preview URL. Only *.preview.localhost origins are allowed.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByTitle("HTML preview for index.html")).not.toBeInTheDocument();
    await waitFor(() =>
      expect(requestJsonMock).toHaveBeenCalledWith(
        expect.stringMatching(
          /\/api\/workspaces\/workspace-1\/preview\/sessions\/[a-z0-9]+$/,
        ),
        expect.objectContaining({ method: "DELETE" }),
      ),
    );
  });

  it("shows session create errors and supports retry", async () => {
    let shouldFail = true;
    mockPreviewApi({
      create: () => {
        if (shouldFail) {
          throw new Error("preview session not found");
        }
        return previewSession("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
      },
    });

    render(<WorkspaceHtmlPreviewPanel tab={tab} />);

    expect(
      await screen.findByText(
        "HTML preview session expired. Refresh to create a new session.",
      ),
    ).toBeInTheDocument();

    shouldFail = false;
    await userEvent.click(screen.getByRole("button", { name: "Retry HTML preview" }));

    expect(await screen.findByTitle("HTML preview for index.html")).toHaveAttribute(
      "src",
      "http://bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.preview.localhost:3210/index.html",
    );
  });

  it("refresh recreates the session and releases the previous token", async () => {
    let createCount = 0;
    mockPreviewApi({
      create: () => {
        createCount += 1;
        return previewSession(
          `t${String(createCount).padStart(31, "0")}`.slice(0, 32),
        );
      },
    });

    render(<WorkspaceHtmlPreviewPanel tab={tab} />);
    const iframe = await screen.findByTitle("HTML preview for index.html");
    const firstSrc = iframe.getAttribute("src");
    expect(firstSrc).toMatch(/\.preview\.localhost:3210\/index\.html$/);
    const firstToken = firstSrc?.match(/http:\/\/([a-z0-9]+)\.preview\.localhost/)?.[1];
    expect(firstToken).toBeTruthy();

    await userEvent.click(
      screen.getByRole("button", { name: "Refresh HTML preview" }),
    );

    await waitFor(() =>
      expect(screen.getByTitle("HTML preview for index.html")).not.toHaveAttribute(
        "src",
        firstSrc,
      ),
    );
    expect(requestJsonMock).toHaveBeenCalledWith(
      `/api/workspaces/workspace-1/preview/sessions/${firstToken}`,
      expect.objectContaining({ method: "DELETE" }),
    );
  });

  it("releases the session on unmount", async () => {
    mockPreviewApi({
      create: () => previewSession(),
    });

    const { unmount } = render(<WorkspaceHtmlPreviewPanel tab={tab} />);
    await waitFor(() =>
      expect(requestJsonMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/preview/sessions",
        expect.objectContaining({ method: "POST" }),
      ),
    );

    // Allow the create promise to settle so tokenRef is populated before unmount.
    await screen.findByTitle("HTML preview for index.html");
    unmount();

    await waitFor(() =>
      expect(requestJsonMock).toHaveBeenCalledWith(
        "/api/workspaces/workspace-1/preview/sessions/abcdefghijklmnopqrstuvwxyz012345",
        expect.objectContaining({ method: "DELETE" }),
      ),
    );
  });
});
