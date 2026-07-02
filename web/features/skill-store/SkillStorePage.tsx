import {
  CheckCircle2,
  Download,
  ExternalLink,
  FileText,
  Folder,
  Languages,
  LoaderCircle,
  RefreshCw,
  Search,
  ShieldCheck,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type {
  ConfiguredSkillSummary,
  SettingsResponse,
  WorkspaceSummary,
} from "../../api/types";
import { errorMessage, requestJson } from "../../shared/api-client";
import { useI18n } from "../../shared/i18n";
import { MarkdownRenderer } from "../chat/MarkdownRenderer";
import "./skill-store.css";

type SkillStorePageProps = {
  onSettingsChange: (settings: SettingsResponse) => void;
  onWorkspacesChange: () => Promise<void> | void;
  settings: SettingsResponse | null;
  workspaces: WorkspaceSummary[];
};

type SkillStoreSkill = {
  id: string;
  name: string;
  description: string;
  source: string | null;
  installs: number | null;
  installsYesterday: number | null;
  change: number | null;
  official: boolean;
};

type SkillStoreFile = {
  path: string;
  content: string;
};

type SkillStoreListResponse = {
  skills: SkillStoreSkill[];
  total: number;
  hasMore: boolean;
  source: string;
};

type SkillStoreDetailResponse = {
  id: string;
  name: string;
  description: string;
  source: string | null;
  files: SkillStoreFile[];
};

type SkillStoreInstallResponse = {
  target: string;
  workspaceId: string | null;
  path: string;
  detected: ConfiguredSkillSummary[];
};

type SkillStoreTranslateResponse = {
  translatedContent: string;
};

type InstallTarget = "global" | "workspace";

type FileTreeNode =
  | {
      children: FileTreeNode[];
      name: string;
      path: string;
      type: "directory";
    }
  | {
      file: SkillStoreFile;
      name: string;
      path: string;
      type: "file";
    };

const SEARCH_DEBOUNCE_MS = 300;

export function SkillStorePage({
  onSettingsChange,
  onWorkspacesChange,
  settings,
  workspaces,
}: SkillStorePageProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [skills, setSkills] = useState<SkillStoreSkill[]>([]);
  const [listSource, setListSource] = useState("");
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const [detail, setDetail] = useState<SkillStoreDetailResponse | null>(null);
  const [isLoadingList, setIsLoadingList] = useState(true);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [detailReloadKey, setDetailReloadKey] = useState(0);
  const [listError, setListError] = useState<string | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [installTarget, setInstallTarget] = useState<InstallTarget>("global");
  const [workspaceId, setWorkspaceId] = useState("");
  const [overwrite, setOverwrite] = useState(false);
  const [isInstalling, setIsInstalling] = useState(false);
  const [installMessage, setInstallMessage] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const [translatedSummary, setTranslatedSummary] = useState<string | null>(null);
  const [isTranslated, setIsTranslated] = useState(false);
  const [isTranslating, setIsTranslating] = useState(false);
  const [translationError, setTranslationError] = useState<string | null>(null);
  const [translationCacheKey, setTranslationCacheKey] = useState<string | null>(null);
  const lastTranslationResetKeyRef = useRef<string | null>(null);

  const configuredWorkspaces = settings?.workspaces ?? [];
  const workspaceTargets = useMemo(() => {
    const items = configuredWorkspaces.length ? configuredWorkspaces : workspaces;
    return items.map((workspace) => ({
      id: workspace.id,
      name: workspace.name,
      path: workspace.path,
    }));
  }, [configuredWorkspaces, workspaces]);

  const selectedSkill = useMemo(
    () => skills.find((skill) => skill.id === selectedSkillId) ?? null,
    [selectedSkillId, skills],
  );

  useEffect(() => {
    if (!workspaceId && workspaceTargets[0]) {
      setWorkspaceId(workspaceTargets[0].id);
    }
  }, [workspaceId, workspaceTargets]);

  useEffect(() => {
    const handle = window.setTimeout(
      () => setDebouncedQuery(query.trim()),
      SEARCH_DEBOUNCE_MS,
    );
    return () => window.clearTimeout(handle);
  }, [query]);

  const loadSkills = useCallback(async () => {
    setIsLoadingList(true);
    setListError(null);
    setInstallMessage(null);
    try {
      const params = new URLSearchParams();
      let path = "/api/skill-store/hot";
      if (debouncedQuery) {
        params.set("query", debouncedQuery);
        path = `/api/skill-store/search?${params.toString()}`;
      }
      const data = await requestJson<SkillStoreListResponse>(path);
      setSkills(data.skills);
      setListSource(data.source);
      setSelectedSkillId((current) =>
        current && data.skills.some((skill) => skill.id === current)
          ? current
          : data.skills[0]?.id ?? null,
      );
    } catch (requestError) {
      setSkills([]);
      setSelectedSkillId(null);
      setListSource("");
      setListError(errorMessage(requestError));
    } finally {
      setIsLoadingList(false);
    }
  }, [debouncedQuery]);

  useEffect(() => {
    void loadSkills();
  }, [loadSkills]);

  useEffect(() => {
    if (!selectedSkill) {
      setDetail(null);
      setDetailError(null);
      return;
    }

    const abortController = new AbortController();
    setIsLoadingDetail(true);
    setDetail(null);
    setDetailError(null);
    setInstallError(null);
    setInstallMessage(null);
    setOverwrite(false);

    const params = new URLSearchParams();
    if (selectedSkill.source) {
      params.set("source", selectedSkill.source);
    }
    const suffix = params.toString() ? `?${params.toString()}` : "";
    requestJson<SkillStoreDetailResponse>(
      `/api/skill-store/skills/${encodeURIComponent(selectedSkill.id)}${suffix}`,
      { signal: abortController.signal },
    )
      .then((data) => setDetail(data))
      .catch((requestError) => {
        if (!abortController.signal.aborted) {
          setDetailError(errorMessage(requestError));
        }
      })
      .finally(() => {
        if (!abortController.signal.aborted) {
          setIsLoadingDetail(false);
        }
      });

    return () => abortController.abort();
  }, [detailReloadKey, selectedSkill]);

  async function installSelectedSkill() {
    if (!detail) {
      return;
    }

    setIsInstalling(true);
    setInstallError(null);
    setInstallMessage(null);
    try {
      const data = await requestJson<SkillStoreInstallResponse>(
        "/api/skill-store/install",
        {
          body: JSON.stringify({
            files: detail.files,
            overwrite,
            skillId: detail.id,
            source: detail.source ?? selectedSkill?.source ?? undefined,
            target: installTarget,
            workspaceId: installTarget === "workspace" ? workspaceId : undefined,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      const refreshedSettings = await requestJson<SettingsResponse>(
        "/api/skills/refresh",
        { method: "POST" },
      );
      onSettingsChange(refreshedSettings);
      await onWorkspacesChange();
      setInstallMessage(t("Installed skill to {path}", { path: data.path }));
      setOverwrite(false);
    } catch (requestError) {
      setInstallError(errorMessage(requestError));
    } finally {
      setIsInstalling(false);
    }
  }

  const skillSummaryFile = detail?.files.find((file) => file.path === "SKILL.md") ?? null;
  const summaryText = skillSummaryFile?.content.trim() ?? "";
  const translationModelId = settings?.skills.translationModelId ?? null;
  const targetLanguage = settings?.general.language ?? "en";
  const summaryTranslationKey = `${detail?.id ?? ""}\u0000${detail?.source ?? ""}\u0000${targetLanguage}\u0000${translationModelId ?? ""}\u0000${summaryText}`;
  const hasCurrentTranslation = Boolean(
    translatedSummary && translationCacheKey === summaryTranslationKey,
  );
  const showingTranslatedSummary = isTranslated && hasCurrentTranslation;
  const displaySummaryText = showingTranslatedSummary && translatedSummary ? translatedSummary : summaryText;
  const canTranslateSummary = Boolean(translationModelId && summaryText);
  const fileTree = useMemo(() => buildFileTree(detail?.files ?? []), [detail?.files]);
  const canInstall = Boolean(
    detail && (installTarget === "global" || workspaceId) && !isInstalling,
  );
  const showOverwriteOption = Boolean(
    installError?.toLocaleLowerCase().includes("already exists") || overwrite,
  );

  useEffect(() => {
    if (lastTranslationResetKeyRef.current === summaryTranslationKey) {
      return;
    }

    lastTranslationResetKeyRef.current = summaryTranslationKey;
    setTranslatedSummary(null);
    setIsTranslated(false);
    setIsTranslating(false);
    setTranslationError(null);
    setTranslationCacheKey(null);
  }, [summaryTranslationKey]);

  async function toggleSummaryTranslation() {
    if (showingTranslatedSummary) {
      setIsTranslated(false);
      setTranslationError(null);
      return;
    }

    if (hasCurrentTranslation) {
      setIsTranslated(true);
      setTranslationError(null);
      return;
    }

    if (!summaryText) {
      return;
    }

    setIsTranslating(true);
    setTranslationError(null);
    const requestKey = summaryTranslationKey;
    try {
      const data = await requestJson<SkillStoreTranslateResponse>(
        "/api/skill-store/translate",
        {
          body: JSON.stringify({
            content: summaryText,
            targetLanguage,
          }),
          headers: { "Content-Type": "application/json" },
          method: "POST",
        },
      );
      if (lastTranslationResetKeyRef.current !== requestKey) {
        return;
      }
      setTranslatedSummary(data.translatedContent);
      setTranslationCacheKey(requestKey);
      setIsTranslated(true);
    } catch (requestError) {
      if (lastTranslationResetKeyRef.current === requestKey) {
        setTranslationError(
          `${t("Could not translate summary")}: ${errorMessage(requestError)}`,
        );
      }
    } finally {
      if (lastTranslationResetKeyRef.current === requestKey) {
        setIsTranslating(false);
      }
    }
  }

  return (
    <section className="skill-store-page">
      <header className="skill-store-header">
        <div className="min-w-0">
          <h1>{t("Skill Store")}</h1>
          <p>{t("Browse skills.sh hot skills from the last 24 hours")}</p>
        </div>
        <button
          aria-label={t("Refresh skills")}
          className="skill-store-icon-button"
          disabled={isLoadingList}
          onClick={() => void loadSkills()}
          title={t("Refresh skills")}
          type="button"
        >
          <RefreshCw
            aria-hidden="true"
            className={`size-4 ${isLoadingList ? "animate-spin" : ""}`}
          />
        </button>
      </header>

      <div className="skill-store-layout">
        <section className="skill-store-list-pane" aria-label={t("Skill list")}>
          <label className="skill-store-search" htmlFor="skill-store-search">
            <Search aria-hidden="true" className="size-4" />
            <input
              id="skill-store-search"
              aria-label={t("Search skills")}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t("Search skills")}
              type="search"
              value={query}
            />
          </label>
          <div className="skill-store-list-meta">
            <span>
              {debouncedQuery
                ? t("Search results")
                : t("Hot skills in the last 24h")}
            </span>
            {listSource ? <code>{listSource}</code> : null}
          </div>

          {listError ? (
            <StatusBlock
              actionLabel={t("Retry")}
              message={listError}
              onAction={() => void loadSkills()}
              title={t("Could not load skills")}
            />
          ) : isLoadingList ? (
            <LoadingBlock label={t("Loading skills...")} />
          ) : skills.length ? (
            <ol className="skill-store-list">
              {skills.map((skill, index) => (
                <li key={`${skill.source ?? "skills"}/${skill.id}`}>
                  <button
                    className={
                      selectedSkillId === skill.id
                        ? "skill-store-row skill-store-row-active"
                        : "skill-store-row"
                    }
                    onClick={() => setSelectedSkillId(skill.id)}
                    type="button"
                  >
                    <span className="skill-store-rank">#{index + 1}</span>
                    <span className="skill-store-row-main">
                      <span className="skill-store-row-title">
                        {skill.name || skill.id}
                        {skill.official ? (
                          <span className="skill-store-official">
                            <ShieldCheck aria-hidden="true" className="size-3" />
                            {t("Official")}
                          </span>
                        ) : null}
                      </span>
                      <span className="skill-store-row-description">
                        {skill.description || skill.id}
                      </span>
                      <span className="skill-store-row-source">
                        {skill.source ?? t("Unknown source")}
                      </span>
                    </span>
                    <span className="skill-store-row-stats">
                      <span>{formatMetric(skill.installs)}</span>
                      <span className={metricChangeClass(skill.change)}>
                        {formatChange(skill.change)}
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ol>
          ) : (
            <StatusBlock
              message={t("No skills matched this query")}
              title={t("No skills")}
            />
          )}
        </section>

        <section className="skill-store-detail-pane" aria-label={t("Skill details")}>
          {!selectedSkill ? (
            <StatusBlock
              message={t("Select a skill from the list")}
              title={t("No skill selected")}
            />
          ) : isLoadingDetail ? (
            <LoadingBlock label={t("Loading skill details...")} />
          ) : detailError ? (
            <StatusBlock
              actionLabel={t("Retry")}
              message={detailError}
              onAction={() => setDetailReloadKey((current) => current + 1)}
              title={t("Could not load skill details")}
            />
          ) : detail ? (
            <div className="skill-store-detail-content">
              <div className="skill-store-detail-heading">
                <div className="min-w-0">
                  <h2>{detail.name || detail.id}</h2>
                  <p>{detail.description || selectedSkill.description}</p>
                  {detail.source ? (
                    <a
                      href={`https://github.com/${detail.source}`}
                      rel="noreferrer"
                      target="_blank"
                    >
                      <ExternalLink aria-hidden="true" className="size-3.5" />
                      {detail.source}
                    </a>
                  ) : null}
                </div>
              </div>

              <div className="skill-store-install-panel">
                <div className="skill-store-install-targets">
                  <button
                    className="skill-store-primary-button skill-store-install-button"
                    disabled={!canInstall}
                    onClick={() => void installSelectedSkill()}
                    type="button"
                  >
                    {isInstalling ? (
                      <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                    ) : (
                      <Download aria-hidden="true" className="size-4" />
                    )}
                    {t("Install")}
                  </button>
                  <label>
                    <span>{t("Install target")}</span>
                    <select
                      value={installTarget}
                      onChange={(event) =>
                        setInstallTarget(event.target.value as InstallTarget)
                      }
                    >
                      <option value="global">{t("Global")}</option>
                      <option value="workspace">{t("Workspace")}</option>
                    </select>
                  </label>
                  {installTarget === "workspace" ? (
                    <label>
                      <span>{t("Workspace")}</span>
                      <select
                        value={workspaceId}
                        onChange={(event) => setWorkspaceId(event.target.value)}
                      >
                        {workspaceTargets.map((workspace) => (
                          <option key={workspace.id} value={workspace.id}>
                            {workspace.name}
                          </option>
                        ))}
                      </select>
                    </label>
                  ) : null}
                </div>
                {showOverwriteOption ? (
                  <label className="skill-store-overwrite">
                    <input
                      checked={overwrite}
                      onChange={(event) => setOverwrite(event.target.checked)}
                      type="checkbox"
                    />
                    <span>{t("Overwrite existing skill")}</span>
                  </label>
                ) : null}
                {installMessage ? (
                  <div className="skill-store-install-actions">
                    <span className="skill-store-success" role="status">
                      <CheckCircle2 aria-hidden="true" className="size-4" />
                      {installMessage}
                    </span>
                  </div>
                ) : null}
                {installError ? (
                  <p className="skill-store-error" role="alert">
                    {installError}
                  </p>
                ) : null}
              </div>

              <section className="skill-store-detail-section">
                <div className="skill-store-section-heading">
                  <h3>{t("Summary")}</h3>
                  {canTranslateSummary ? (
                    <button
                      aria-label={
                        isTranslating
                          ? t("Translating")
                          : showingTranslatedSummary
                            ? t("Show original")
                            : t("Translate")
                      }
                      className="skill-store-icon-button"
                      disabled={isTranslating}
                      onClick={() => void toggleSummaryTranslation()}
                      title={
                        isTranslating
                          ? t("Translating")
                          : showingTranslatedSummary
                            ? t("Show original")
                            : t("Translate")
                      }
                      type="button"
                    >
                      {isTranslating ? (
                        <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                      ) : showingTranslatedSummary ? (
                        <RefreshCw aria-hidden="true" className="size-4" />
                      ) : (
                        <Languages aria-hidden="true" className="size-4" />
                      )}
                    </button>
                  ) : null}
                </div>
                {translationError ? (
                  <p className="skill-store-translation-error" role="alert">
                    {translationError}
                  </p>
                ) : null}
                {summaryText ? (
                  <div className="markdown-content skill-store-summary-markdown">
                    <MarkdownRenderer allowHtml={false} content={displaySummaryText} />
                  </div>
                ) : (
                  <p className="skill-store-detail-empty">{t("No summary available")}</p>
                )}
              </section>

              <section className="skill-store-detail-section">
                <div className="skill-store-section-heading">
                  <h3>{t("Files")}</h3>
                </div>
                <FileTree nodes={fileTree} />
              </section>
            </div>
          ) : null}
        </section>
      </div>
    </section>
  );
}

function FileTree({ nodes }: { nodes: FileTreeNode[] }) {
  return (
    <ul className="skill-store-file-list">
      {nodes.map((node) => (
        <li key={`${node.type}:${node.path}`}>
          <div className="skill-store-file-tree-row" title={node.path}>
            {node.type === "directory" ? (
              <Folder aria-hidden="true" className="size-4" />
            ) : (
              <FileText aria-hidden="true" className="size-4" />
            )}
            <span>{node.name}</span>
            {node.type === "file" ? <code>{formatBytes(node.file.content.length)}</code> : null}
          </div>
          {node.type === "directory" ? <FileTree nodes={node.children} /> : null}
        </li>
      ))}
    </ul>
  );
}

function buildFileTree(files: SkillStoreFile[]): FileTreeNode[] {
  type FileNode = Extract<FileTreeNode, { type: "file" }>;
  type MutableDirectory = {
    children: Map<string, MutableDirectory | FileNode>;
    name: string;
    path: string;
    type: "directory";
  };

  const root: MutableDirectory = {
    children: new Map(),
    name: "",
    path: "",
    type: "directory",
  };

  for (const file of files) {
    const parts = file.path.split("/").filter(Boolean);
    let directory = root;
    for (const [index, part] of parts.entries()) {
      const path = parts.slice(0, index + 1).join("/");
      if (index === parts.length - 1) {
        directory.children.set(part, { file, name: part, path: file.path, type: "file" });
        continue;
      }

      const existing = directory.children.get(part);
      if (existing?.type === "directory") {
        directory = existing;
        continue;
      }

      const next: MutableDirectory = {
        children: new Map(),
        name: part,
        path,
        type: "directory",
      };
      directory.children.set(part, next);
      directory = next;
    }
  }

  function freeze(directory: MutableDirectory): FileTreeNode[] {
    return Array.from(directory.children.values()).map((node) => {
      if (node.type === "file") {
        return node;
      }
      return {
        children: freeze(node),
        name: node.name,
        path: node.path,
        type: "directory",
      };
    });
  }

  return freeze(root);
}

function LoadingBlock({ label }: { label: string }) {
  return (
    <div className="skill-store-status-block" role="status">
      <LoaderCircle aria-hidden="true" className="size-5 animate-spin" />
      <span>{label}</span>
    </div>
  );
}

function StatusBlock({
  actionLabel,
  message,
  onAction,
  title,
}: {
  actionLabel?: string;
  message: string;
  onAction?: () => void;
  title: string;
}) {
  return (
    <div className="skill-store-status-block">
      <strong>{title}</strong>
      <span>{message}</span>
      {actionLabel && onAction ? (
        <button onClick={onAction} type="button">
          {actionLabel}
        </button>
      ) : null}
    </div>
  );
}

function formatMetric(value: number | null) {
  return typeof value === "number" ? value.toLocaleString() : "-";
}

function formatChange(value: number | null) {
  if (typeof value !== "number") {
    return "-";
  }
  return value > 0 ? `+${value.toLocaleString()}` : value.toLocaleString();
}

function metricChangeClass(value: number | null) {
  if (typeof value !== "number") {
    return "skill-store-change";
  }
  if (value > 0) {
    return "skill-store-change skill-store-change-positive";
  }
  if (value < 0) {
    return "skill-store-change skill-store-change-negative";
  }
  return "skill-store-change";
}

function formatBytes(value: number) {
  if (value < 1024) {
    return `${value} B`;
  }
  return `${Math.round(value / 102.4) / 10} KB`;
}
