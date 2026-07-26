import { AppWindow, LoaderCircle, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { PreviewSessionResponse } from "../../api/types";
import { errorMessage, requestJson } from "../../shared/api-client";
import { useI18n } from "../../shared/i18n";
import { Button } from "../../shared/ui";

export type OpenHtmlPreviewTab = {
  workspaceId: string;
  path: string;
  name: string;
  workspaceName: string;
  workspaceLogoUrl: string | null;
};

/** Host-mode sandbox: independent preview origin may use same-origin within that origin. */
export const HTML_PREVIEW_IFRAME_SANDBOX_HOST = "allow-scripts allow-same-origin";
/** Path-mode sandbox: omit allow-same-origin so the iframe gets an opaque origin. */
export const HTML_PREVIEW_IFRAME_SANDBOX_PATH = "allow-scripts";
/** @deprecated Prefer resolveHtmlPreviewIframeSandbox; host-mode default. */
export const HTML_PREVIEW_IFRAME_SANDBOX = HTML_PREVIEW_IFRAME_SANDBOX_HOST;

const PREVIEW_TOKEN_RE = /^[a-z0-9]{1,63}$/;

function isDnsSafePreviewToken(token: string): boolean {
  return PREVIEW_TOKEN_RE.test(token) && !token.includes(".");
}

/** True when previewUrl is host-mode (`*.preview.localhost`) or same-origin path-mode (`/__preview/{token}/...`). */
export function isSafeHtmlPreviewUrl(previewUrl: string): boolean {
  try {
    const url = new URL(previewUrl, window.location.origin);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      return false;
    }

    const hostname = url.hostname.toLowerCase();
    if (hostname.endsWith(".preview.localhost")) {
      const token = hostname.slice(0, -".preview.localhost".length);
      return isDnsSafePreviewToken(token);
    }

    // Path mode: same host as the Foco UI, capability only in the path token.
    if (hostname !== window.location.hostname.toLowerCase()) {
      return false;
    }
    const match = url.pathname.match(/^\/__preview\/([a-z0-9]{1,63})(?:\/|$)/);
    return Boolean(match && isDnsSafePreviewToken(match[1]));
  } catch {
    return false;
  }
}

export function isPathModeHtmlPreviewUrl(previewUrl: string): boolean {
  try {
    const url = new URL(previewUrl, window.location.origin);
    return (
      url.hostname.toLowerCase() === window.location.hostname.toLowerCase() &&
      /^\/__preview\/[a-z0-9]{1,63}(?:\/|$)/.test(url.pathname)
    );
  } catch {
    return false;
  }
}

/** Client-pinned sandbox from URL shape. Never trust server-provided sandbox strings. */
export function resolveHtmlPreviewIframeSandbox(previewUrl: string): string {
  return isPathModeHtmlPreviewUrl(previewUrl)
    ? HTML_PREVIEW_IFRAME_SANDBOX_PATH
    : HTML_PREVIEW_IFRAME_SANDBOX_HOST;
}

export function WorkspaceHtmlPreviewPanel({
  tab,
}: {
  tab: OpenHtmlPreviewTab;
}) {
  const { t } = useI18n();
  const [session, setSession] = useState<PreviewSessionResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [iframeReloadKey, setIframeReloadKey] = useState(0);
  const tokenRef = useRef<string | null>(null);
  const generationRef = useRef(0);
  const translateRef = useRef(t);
  const workspaceIdRef = useRef(tab.workspaceId);
  translateRef.current = t;
  workspaceIdRef.current = tab.workspaceId;

  const releaseSession = useCallback(async (workspaceId: string, token: string) => {
    try {
      await requestJson(
        `/api/workspaces/${encodeURIComponent(workspaceId)}/preview/sessions/${encodeURIComponent(token)}`,
        { method: "DELETE" },
      );
    } catch {
      // Best-effort: session may already be expired or the process restarted.
    }
  }, []);

  const createSession = useCallback(
    async (options: { recreate?: boolean } = {}) => {
      const generation = ++generationRef.current;
      setIsLoading(true);
      setError(null);

      const previousToken = tokenRef.current;
      if (options.recreate && previousToken) {
        tokenRef.current = null;
        void releaseSession(tab.workspaceId, previousToken);
      }

      try {
        const next = await requestJson<PreviewSessionResponse>(
          `/api/workspaces/${encodeURIComponent(tab.workspaceId)}/preview/sessions`,
          {
            body: JSON.stringify({ path: tab.path }),
            headers: { "Content-Type": "application/json" },
            method: "POST",
          },
        );

        if (generation !== generationRef.current) {
          void releaseSession(tab.workspaceId, next.token);
          return;
        }

        if (!isSafeHtmlPreviewUrl(next.previewUrl)) {
          void releaseSession(tab.workspaceId, next.token);
          tokenRef.current = null;
          setSession(null);
          setError(
            translateRef.current(
              "HTML preview returned an unsafe preview URL. Only *.preview.localhost origins are allowed.",
            ),
          );
          setIsLoading(false);
          return;
        }

        tokenRef.current = next.token;
        setSession(next);
        setIframeReloadKey((current) => current + 1);
        setIsLoading(false);
      } catch (requestError) {
        if (generation !== generationRef.current) {
          return;
        }

        tokenRef.current = null;
        setSession(null);
        setError(mapPreviewSessionError(errorMessage(requestError), translateRef.current));
        setIsLoading(false);
      }
    },
    [releaseSession, tab.path, tab.workspaceId],
  );

  useEffect(() => {
    void createSession();

    return () => {
      generationRef.current += 1;
      const token = tokenRef.current;
      tokenRef.current = null;
      if (token) {
        void releaseSession(workspaceIdRef.current, token);
      }
    };
  }, [createSession, releaseSession]);

  function handleRefresh() {
    // Always reissue a session so idle/expired tokens are replaced.
    void createSession({ recreate: true });
  }

  function handleIframeError() {
    setError(t("HTML preview failed to load. Refresh or recreate the session."));
  }

  return (
    <section className="workspace-html-preview flex min-h-0 flex-1 flex-col">
      <div
        aria-label={t("HTML preview toolbar")}
        className="workspace-html-preview-toolbar"
        role="toolbar"
      >
        <Button
          aria-label={t("Refresh HTML preview")}
          className="workspace-file-editor-toolbar-button size-7 min-w-7"
          isDisabled={isLoading}
          isIconOnly
          onPress={handleRefresh}
          size="sm"
          variant="ghost"
        >
          <RefreshCw
            aria-hidden="true"
            className={`size-4 ${isLoading ? "animate-spin" : ""}`}
          />
        </Button>
        <span className="workspace-html-preview-path" title={tab.path}>
          {tab.path}
        </span>
      </div>

      {error ? (
        <div className="workspace-html-preview-status workspace-html-preview-status-error">
          <p>{error}</p>
          <Button
            className="workspace-html-preview-retry"
            onPress={() => void createSession({ recreate: true })}
          >
            {t("Retry HTML preview")}
          </Button>
        </div>
      ) : null}

      {isLoading && !session ? (
        <div className="workspace-html-preview-status" role="status">
          <LoaderCircle aria-hidden="true" className="size-5 animate-spin text-[var(--accent-soft-foreground)]" />
          <span>{t("Loading HTML preview…")}</span>
        </div>
      ) : null}

      {session && !error ? (
        <iframe
          className="workspace-html-preview-frame"
          key={`${session.token}:${iframeReloadKey}`}
          onError={handleIframeError}
          referrerPolicy="no-referrer"
          sandbox={resolveHtmlPreviewIframeSandbox(session.previewUrl)}
          src={session.previewUrl}
          title={t("HTML preview for {name}", { name: tab.name })}
        />
      ) : null}

      {!isLoading && !session && !error ? (
        <div className="workspace-html-preview-status">
          <AppWindow aria-hidden="true" className="size-5 text-[var(--muted)]" />
          <span>{t("HTML preview is unavailable.")}</span>
        </div>
      ) : null}
    </section>
  );
}

function mapPreviewSessionError(
  message: string,
  t: (key: string, values?: Record<string, string | number>) => string,
) {
  const lower = message.toLowerCase();
  if (
    lower.includes("offline") ||
    lower.includes("not connected") ||
    lower.includes("remote workspace") ||
    lower.includes("sidecar") ||
    lower.includes("502")
  ) {
    return t("Remote workspace is offline or the preview connection failed.");
  }

  if (
    lower.includes("expired") ||
    lower.includes("unknown preview") ||
    lower.includes("invalid preview") ||
    lower.includes("preview session")
  ) {
    return t("HTML preview session expired. Refresh to create a new session.");
  }

  // Match backend entry validation specifically; avoid broad "html"/"htm" substrings.
  if (
    lower.includes(".html") ||
    lower.includes(".htm") ||
    lower.includes("must be an .html") ||
    lower.includes("must be a .html") ||
    lower.includes("not a previewable") ||
    lower.includes("unsupported preview")
  ) {
    return t("Only .html and .htm files can be opened in the HTML preview tab.");
  }

  if (lower.includes("limit")) {
    return t("Too many open HTML previews. Close idle previews and try again.");
  }

  return message || t("Failed to create HTML preview session.");
}
