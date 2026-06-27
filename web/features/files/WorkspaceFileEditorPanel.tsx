import {
  ClipboardPaste,
  Copy,
  Eye,
  EyeOff,
  Redo2,
  RefreshCw,
  Save,
  Scissors,
  Search,
  Undo2,
  WrapText,
  type LucideIcon,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import type * as Monaco from "monaco-editor";

import { MarkdownContent } from "../chat/MarkdownContent";
import { errorMessage } from "../../shared/api-client";
import { useI18n } from "../../shared/i18n";

export type OpenFileTab = {
  workspaceId: string;
  path: string;
  name: string;
  workspaceName: string;
  workspaceLogoUrl: string | null;
};

export type WorkspaceFileEditorState = {
  content: string;
  error: string | null;
  isDirty: boolean;
  isLoading: boolean;
  isSaving: boolean;
  lastSavedContent: string;
};

type NetworkInformationLike = {
  saveData?: boolean;
};

type WindowWithIdleCallback = Window & {
  cancelIdleCallback?: (handle: number) => void;
  requestIdleCallback?: (
    callback: (deadline: { didTimeout: boolean; timeRemaining: () => number }) => void,
    options?: { timeout?: number },
  ) => number;
};

const WORKSPACE_IMAGE_FILE_EXTENSIONS = new Set(["gif", "jpeg", "jpg", "png", "svg", "webp"]);
const markdownSelectedSkillPrefix = () => null;

let monacoPreloadPromise: Promise<typeof import("monaco-editor")> | null = null;

function preloadMonaco() {
  monacoPreloadPromise ??= import("monaco-editor").catch((error: unknown) => {
    monacoPreloadPromise = null;
    throw error;
  });
  return monacoPreloadPromise;
}

function preloadMonacoQuietly() {
  void preloadMonaco().catch(() => undefined);
}

function canPreloadOptionalMonaco() {
  if (typeof navigator === "undefined") {
    return false;
  }

  const connection = (navigator as Navigator & { connection?: NetworkInformationLike }).connection;
  return connection?.saveData !== true;
}

export function scheduleOptionalMonacoPreload() {
  if (typeof window === "undefined" || !canPreloadOptionalMonaco()) {
    return undefined;
  }

  const idleWindow = window as WindowWithIdleCallback;
  if (idleWindow.requestIdleCallback) {
    const handle = idleWindow.requestIdleCallback(preloadMonacoQuietly, { timeout: 5000 });
    return () => idleWindow.cancelIdleCallback?.(handle);
  }

  const handle = window.setTimeout(preloadMonacoQuietly, 3000);
  return () => window.clearTimeout(handle);
}

export function preloadOptionalMonaco() {
  if (canPreloadOptionalMonaco()) {
    preloadMonacoQuietly();
  }
}

export function WorkspaceFileEditorPanel({
  editor,
  file,
  onChangeContent,
  onReload,
  onSave,
}: {
  editor: WorkspaceFileEditorState | null;
  file: OpenFileTab;
  onChangeContent: (workspaceId: string, path: string, content: string) => void;
  onReload: (file: OpenFileTab) => Promise<void>;
  onSave: (file: OpenFileTab, content: string) => Promise<boolean> | boolean;
}) {
  const isImage = isWorkspaceImageFilePath(file.path);
  const imageUrl = isImage ? workspaceFileBlobUrl(file.workspaceId, file.path) : null;
  const language = monacoLanguageForPath(file.path);
  const isMarkdown = isMarkdownFilePath(file.path);
  const editorPath = `${file.workspaceId}/${file.path}`;
  const handleChange = useCallback(
    (content: string) => onChangeContent(file.workspaceId, file.path, content),
    [file.path, file.workspaceId, onChangeContent],
  );
  const handleReload = useCallback(() => onReload(file), [file, onReload]);
  const handleSave = useCallback(
    (content: string) => onSave(file, content),
    [file, onSave],
  );

  return (
    <section className="workspace-file-editor flex min-h-0 flex-1 flex-col">
      {editor?.error ? (
        <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {editor.error}
        </div>
      ) : null}
      <div className="workspace-file-editor-body">
        {imageUrl ? (
          <WorkspaceImagePreview alt={file.name} src={imageUrl} />
        ) : (
          <MonacoFileEditor
            canSave={!editor?.isLoading && !editor?.isSaving}
            isDirty={editor?.isDirty ?? false}
            isMarkdown={isMarkdown}
            isSaving={editor?.isSaving ?? false}
            language={language}
            onChange={handleChange}
            onReload={handleReload}
            onSave={handleSave}
            path={editorPath}
            value={editor?.content ?? ""}
            workspaceFilePath={file.path}
            workspaceId={file.workspaceId}
          />
        )}
      </div>
    </section>
  );
}

function WorkspaceImagePreview({ alt, src }: { alt: string; src: string }) {
  return (
    <div className="workspace-file-image-preview">
      <img alt={alt} className="workspace-file-image-preview-image" src={src} />
    </div>
  );
}

type MonacoFileEditorCommand =
  | "save"
  | "cut"
  | "copy"
  | "paste"
  | "undo"
  | "redo"
  | "find"
  | "toggleWordWrap";

function MonacoFileEditor({
  canSave,
  isDirty,
  isMarkdown,
  isSaving,
  language,
  onChange,
  onReload,
  onSave,
  path,
  value,
  workspaceFilePath,
  workspaceId,
}: {
  canSave: boolean;
  isDirty: boolean;
  isMarkdown: boolean;
  isSaving: boolean;
  language: string;
  onChange: (value: string) => void;
  onReload: () => Promise<void>;
  onSave: (value: string) => Promise<boolean> | boolean;
  path: string;
  value: string;
  workspaceFilePath: string;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null);
  const modelRef = useRef<Monaco.editor.ITextModel | null>(null);
  const ignoreModelChangeRef = useRef(false);
  const valueRef = useRef(value);
  const [previewEnabled, setPreviewEnabled] = useState(false);
  const [wordWrapEnabled, setWordWrapEnabled] = useState(false);
  const [monacoError, setMonacoError] = useState<string | null>(null);
  const [isReloadConfirmOpen, setIsReloadConfirmOpen] = useState(false);
  const [isReloading, setIsReloading] = useState(false);
  const reloadConfirmTitleId = useId();
  const reloadConfirmDescriptionId = useId();

  useEffect(() => {
    valueRef.current = value;
  }, [value]);

  useEffect(() => {
    setPreviewEnabled(false);
  }, [path]);

  useEffect(() => {
    if (!previewEnabled) {
      window.setTimeout(() => editorRef.current?.layout(), 0);
    }
  }, [previewEnabled]);

  const focusEditor = useCallback(() => {
    editorRef.current?.focus();
  }, []);

  const reloadFile = useCallback(async () => {
    if (isReloading) {
      return;
    }

    setIsReloading(true);
    try {
      await onReload();
    } finally {
      setIsReloading(false);
      editorRef.current?.focus();
    }
  }, [isReloading, onReload]);

  const handleReloadClick = useCallback(() => {
    if (!isDirty) {
      void reloadFile();
      return;
    }

    setIsReloadConfirmOpen(true);
  }, [isDirty, reloadFile]);

  const handleReloadConfirm = useCallback(
    async (action: "save" | "discard" | "cancel") => {
      if (action === "cancel") {
        setIsReloadConfirmOpen(false);
        editorRef.current?.focus();
        return;
      }

      setIsReloadConfirmOpen(false);
      if (action === "save") {
        const saveResult = await onSave(editorRef.current?.getValue() ?? valueRef.current);
        if (saveResult === false) {
          editorRef.current?.focus();
          return;
        }
      }

      await reloadFile();
    },
    [onSave, reloadFile],
  );

  const runEditorCommand = useCallback(
    (command: MonacoFileEditorCommand) => {
      if (command === "save") {
        if (canSave) {
          onSave(editorRef.current?.getValue() ?? valueRef.current);
        }
        editorRef.current?.focus();
        return;
      }

      const editor = editorRef.current;
      if (!editor) {
        return;
      }

      if (command === "toggleWordWrap") {
        const nextEnabled = !wordWrapEnabled;
        editor.updateOptions({ wordWrap: nextEnabled ? "on" : "off" });
        setWordWrapEnabled(nextEnabled);
        editor.focus();
        return;
      }

      const commandIdByAction: Record<Exclude<MonacoFileEditorCommand, "save" | "toggleWordWrap">, string> = {
        copy: "editor.action.clipboardCopyAction",
        cut: "editor.action.clipboardCutAction",
        find: "actions.find",
        paste: "editor.action.clipboardPasteAction",
        redo: "redo",
        undo: "undo",
      };
      editor.trigger("workspace-file-toolbar", commandIdByAction[command], null);
      editor.focus();
    },
    [canSave, onSave, wordWrapEnabled],
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return undefined;
    }

    let disposed = false;
    let cleanupEditor: (() => void) | null = null;

    void preloadMonaco()
      .then((monaco) => {
        if (disposed) {
          return;
        }

        setMonacoError(null);
        registerTomlMonacoLanguage(monaco);

        const model = monaco.editor.createModel(
          valueRef.current,
          language,
          monaco.Uri.parse(`file:///${path}`),
        );
        const editor = monaco.editor.create(container, {
          automaticLayout: true,
          fontSize: 13,
          language,
          minimap: { enabled: true },
          model,
          readOnly: false,
          scrollBeyondLastLine: false,
          theme: "vs",
          wordWrap: wordWrapEnabled ? "on" : "off",
        });
        const changeDisposable = model.onDidChangeContent(() => {
          if (!ignoreModelChangeRef.current) {
            onChange(model.getValue());
          }
        });
        editor.addCommand(
          monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS,
          () => {
            onSave(editor.getValue());
          },
        );
        editorRef.current = editor;
        modelRef.current = model;
        cleanupEditor = () => {
          changeDisposable.dispose();
          editor.dispose();
          model.dispose();
        };
      })
      .catch((loadError: unknown) => {
        if (!disposed) {
          setMonacoError(errorMessage(loadError));
        }
      });

    return () => {
      disposed = true;
      cleanupEditor?.();
      editorRef.current = null;
      modelRef.current = null;
    };
  }, [language, onChange, onSave, path]);

  useEffect(() => {
    const model = modelRef.current;
    if (!model || model.getValue() === value) {
      return;
    }
    ignoreModelChangeRef.current = true;
    model.setValue(value);
    ignoreModelChangeRef.current = false;
  }, [value]);

  const markdownImageUrlTransform = useMemo(
    () => workspaceMarkdownImageUrlTransform(workspaceId, workspaceFilePath),
    [workspaceFilePath, workspaceId],
  );

  return (
    <div className="workspace-file-editor-shell">
      <div aria-label={t("Editor toolbar")} className="workspace-file-editor-toolbar" role="toolbar">
        <EditorToolbarButton
          disabled={!canSave || isReloading}
          icon={RefreshCw}
          label={t("Reload file")}
          onClick={handleReloadClick}
        />
        <span className="workspace-file-editor-toolbar-separator" />
        <EditorToolbarButton
          disabled={!canSave || isSaving}
          icon={Save}
          isActive={isDirty}
          label={t("Save")}
          onClick={() => runEditorCommand("save")}
        />
        <span className="workspace-file-editor-toolbar-separator" />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Scissors}
          label={t("Cut")}
          onClick={() => runEditorCommand("cut")}
        />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Copy}
          label={t("Copy")}
          onClick={() => runEditorCommand("copy")}
        />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={ClipboardPaste}
          label={t("Paste")}
          onClick={() => runEditorCommand("paste")}
        />
        <span className="workspace-file-editor-toolbar-separator" />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Undo2}
          label={t("Undo")}
          onClick={() => runEditorCommand("undo")}
        />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Redo2}
          label={t("Redo")}
          onClick={() => runEditorCommand("redo")}
        />
        <span className="workspace-file-editor-toolbar-separator" />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={Search}
          label={t("Find")}
          onClick={() => runEditorCommand("find")}
        />
        <EditorToolbarButton
          disabled={previewEnabled}
          icon={WrapText}
          isActive={wordWrapEnabled}
          label={t("Word wrap")}
          onClick={() => runEditorCommand("toggleWordWrap")}
        />
        {isMarkdown ? (
          <>
            <span className="workspace-file-editor-toolbar-separator" />
            <EditorToolbarButton
              icon={previewEnabled ? EyeOff : Eye}
              isActive={previewEnabled}
              label={previewEnabled ? t("Edit markdown") : t("Preview markdown")}
              onClick={() => setPreviewEnabled((current) => !current)}
            />
          </>
        ) : null}
      </div>
      {monacoError ? (
        <div className="border-b border-rose-200 bg-rose-50 px-4 py-3 text-sm text-rose-700">
          {monacoError}
        </div>
      ) : null}
      <div
        aria-hidden={previewEnabled || undefined}
        className={`workspace-file-monaco ${previewEnabled ? "workspace-file-monaco-hidden" : ""}`}
        onMouseDown={focusEditor}
        ref={containerRef}
      />
      {isMarkdown && previewEnabled ? (
        <div className="workspace-file-markdown-preview">
          <MarkdownContent
            allowHtml
            content={value}
            imageUrlTransform={markdownImageUrlTransform}
            isUser={false}
            selectedSkillPrefix={markdownSelectedSkillPrefix}
          />
        </div>
      ) : null}
      {isReloadConfirmOpen ? (
        <div className="workspace-file-reload-dialog-backdrop">
          <div
            aria-describedby={reloadConfirmDescriptionId}
            aria-labelledby={reloadConfirmTitleId}
            aria-modal="true"
            className="workspace-file-reload-dialog"
            role="dialog"
          >
            <h2 id={reloadConfirmTitleId}>{t("Reload file")}</h2>
            <p id={reloadConfirmDescriptionId}>{t("Save changes before reloading this file?")}</p>
            <div className="workspace-file-reload-dialog-actions">
              <button
                className="rounded-lg bg-stone-900 px-3 py-2 text-sm font-semibold text-white hover:bg-stone-800"
                onClick={() => void handleReloadConfirm("save")}
                type="button"
              >
                {t("Yes")}
              </button>
              <button
                className="rounded-lg border border-stone-300 px-3 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50"
                onClick={() => void handleReloadConfirm("discard")}
                type="button"
              >
                {t("No")}
              </button>
              <button
                className="rounded-lg border border-stone-300 px-3 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50"
                onClick={() => void handleReloadConfirm("cancel")}
                type="button"
              >
                {t("Cancel")}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function EditorToolbarButton({
  disabled = false,
  icon: Icon,
  isActive = false,
  label,
  onClick,
}: {
  disabled?: boolean;
  icon: LucideIcon;
  isActive?: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      aria-pressed={isActive || undefined}
      className={`workspace-file-editor-toolbar-button ${isActive ? "workspace-file-editor-toolbar-button-active" : ""}`}
      disabled={disabled}
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon aria-hidden="true" className="size-4" />
    </button>
  );
}

function isMarkdownFilePath(path: string) {
  const extension = path.split(".").pop()?.toLowerCase();
  return extension === "md" || extension === "markdown";
}

export function isWorkspaceImageFilePath(path: string) {
  const extension = path.split(".").pop()?.toLowerCase();
  return extension ? WORKSPACE_IMAGE_FILE_EXTENSIONS.has(extension) : false;
}

function workspaceFileBlobUrl(workspaceId: string, workspacePath: string) {
  return `/api/workspaces/${encodeURIComponent(workspaceId)}/files/blob?path=${encodeURIComponent(workspacePath)}`;
}

function workspaceMarkdownImageUrlTransform(workspaceId: string, filePath: string) {
  const parentPath = filePath.includes("/") ? filePath.slice(0, filePath.lastIndexOf("/")) : "";
  const basePath = `/__workspace__/${parentPath ? `${parentPath}/` : ""}`;

  return (url: string) => {
    const trimmedUrl = url.trim();
    if (!trimmedUrl || trimmedUrl.startsWith("#") || trimmedUrl.startsWith("/") || /^[a-z][a-z0-9+.-]*:/i.test(trimmedUrl)) {
      return null;
    }

    const normalizedPath = new URL(trimmedUrl, `https://foco.local${basePath}`).pathname;
    if (!normalizedPath.startsWith("/__workspace__/")) {
      return null;
    }

    const workspacePath = normalizedPath.slice("/__workspace__/".length);
    if (!workspacePath) {
      return null;
    }

    return workspaceFileBlobUrl(workspaceId, workspacePath);
  };
}

function registerTomlMonacoLanguage(monaco: typeof Monaco) {
  if (monaco.languages.getLanguages().some((language) => language.id === "toml")) {
    return;
  }

  monaco.languages.register({
    id: "toml",
    aliases: ["TOML", "toml"],
    extensions: [".toml"],
    mimetypes: ["application/toml"],
  });
  monaco.languages.setMonarchTokensProvider("toml", {
    defaultToken: "",
    tokenPostfix: ".toml",
    escapes: /\\(?:[btnfr"\\]|u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8})/,
    tokenizer: {
      root: [
        [/^\s*(\[+)([^\]]+)(\]+)/, ["delimiter.bracket", "type.identifier", "delimiter.bracket"]],
        [/^\s*([A-Za-z0-9_-]+)(\s*=)/, ["key", "delimiter"]],
        { include: "@values" },
        [/#.*$/, "comment"],
      ],
      values: [
        [/"""/, "string", "multilineDoubleString"],
        [/'''/, "string", "multilineSingleString"],
        [/"/, "string", "doubleString"],
        [/'/, "string", "singleString"],
        [/\b(?:true|false)\b/, "keyword"],
        [/\b\d{4}-\d{2}-\d{2}(?:[Tt ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)?\b/, "number.date"],
        [/[+-]?\b(?:0x[0-9A-Fa-f_]+|0o[0-7_]+|0b[01_]+)\b/, "number"],
        [/[+-]?\b\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d[\d_]*)?\b/, "number"],
        [/[\[\]{}.,]/, "delimiter"],
        [/#.*$/, "comment"],
      ],
      doubleString: [
        [/[^\\"#]+/, "string"],
        [/@escapes/, "string.escape"],
        [/\\./, "string.escape.invalid"],
        [/"/, "string", "@pop"],
        [/#.*$/, "comment"],
      ],
      singleString: [
        [/[^']+/, "string"],
        [/'/, "string", "@pop"],
      ],
      multilineDoubleString: [
        [/[^\\"]+/, "string"],
        [/@escapes/, "string.escape"],
        [/\\./, "string.escape.invalid"],
        [/"""/, "string", "@pop"],
        [/"/, "string"],
      ],
      multilineSingleString: [
        [/[^']+/, "string"],
        [/'''/, "string", "@pop"],
        [/'/, "string"],
      ],
    },
  });
}

function monacoLanguageForPath(path: string) {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  const languageByExtension: Record<string, string> = {
    c: "c",
    cc: "cpp",
    cpp: "cpp",
    cs: "csharp",
    css: "css",
    go: "go",
    h: "cpp",
    hpp: "cpp",
    html: "html",
    java: "java",
    js: "javascript",
    json: "json",
    jsx: "javascript",
    kt: "kotlin",
    less: "less",
    lua: "lua",
    markdown: "markdown",
    md: "markdown",
    php: "php",
    py: "python",
    rb: "ruby",
    rs: "rust",
    sass: "scss",
    scss: "scss",
    sh: "shell",
    sql: "sql",
    swift: "swift",
    toml: "toml",
    ts: "typescript",
    tsx: "typescript",
    xml: "xml",
    yaml: "yaml",
    yml: "yaml",
  };

  return languageByExtension[extension] ?? "plaintext";
}
