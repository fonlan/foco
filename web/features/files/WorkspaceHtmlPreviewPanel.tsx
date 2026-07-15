import { AppWindow, LoaderCircle, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { PreviewSessionResponse } from "../../api/types";
import { errorMessage, requestJson } from "../../shared/api-client";
import { useI18n } from "../../shared/i18n";

export type OpenHtmlPreviewTab = {
  workspaceId: string;
  path: string;
  name: string;
  workspaceName: string;
  workspaceLogoUrl: string | null;
};

/** Client-pinned sandbox. Never trust server-provided sandbox strings. */
export const HTML_PREVIEW_IFRAME_SANDBOX = "allow-scripts allow-same-origin";

export function isSafeHtmlPreviewUrl(previewUrl: string): boolean {
  try {
    const url = new URL(previewUrl);
    if (url.protocol !== "http:" && url.protocol !== "https:") {
      return false;
    }
    const hostname = url.hostname.toLowerCase();
    // Align with backend parse_preview_host_token: require a single DNS-safe token label.
    if (!hostname.endsWith(".preview.localhost")) {
      return false;
    }
    const token = hostname.slice(0, -".preview.localhost".length);
    return (
      token.length > 0 &&
      token.length <= 63 &&
      !token.includes(".") &&
      /^[a-z0-9]+$/.test(token)
    );
  } catch {
    return false;
  }
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
  const workspaceIdRef = useRef(tab.workspaceId);
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
            t(
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
        setError(mapPreviewSessionError(errorMessage(requestError), t));
        setIsLoading(false);
      }
    },
    [releaseSession, t, tab.path, tab.workspaceId],
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
        <button
          aria-label={t("Refresh HTML preview")}
          className="workspace-file-editor-toolbar-button"
          disabled={isLoading}
          onClick={handleRefresh}
          title={t("Refresh HTML preview")}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className={`size-4 ${isLoading ? "animate-spin" : ""}`}
          />
        </button>
        <span className="workspace-html-preview-path" title={tab.path}>
          {tab.path}
        </span>
      </div>

      {error ? (
        <div className="workspace-html-preview-status workspace-html-preview-status-error">
          <p>{error}</p>
          <button
            className="workspace-html-preview-retry"
            onClick={() => void createSession({ recreate: true })}
            type="button"
          >
            {t("Retry HTML preview")}
          </button>
        </div>
      ) : null}

      {isLoading && !session ? (
        <div className="workspace-html-preview-status" role="status">
          <LoaderCircle aria-hidden="true" className="size-5 animate-spin text-teal-700" />
          <span>{t("Loading HTML preview...")}</span>
        </div>
      ) : null}

      {session && !error ? (
        <iframe
          className="workspace-html-preview-frame"
          key={`${session.token}:${iframeReloadKey}`}
          onError={handleIframeError}
          referrerPolicy="no-referrer"
          sandbox={HTML_PREVIEW_IFRAME_SANDBOX}
          src={session.previewUrl}
          title={t("HTML preview for {name}", { name: tab.name })}
        />
      ) : null}

      {!isLoading && !session && !error ? (
        <div className="workspace-html-preview-status">
          <AppWindow aria-hidden="true" className="size-5 text-stone-400" />
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
