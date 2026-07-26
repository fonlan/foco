import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { WorkspaceFileEditorPanel } from "./WorkspaceFileEditorPanel";
import type { WorkspaceFileEditorState } from "./WorkspaceFileEditorPanel";

const monacoMock = vi.hoisted(() => {
  const editorDispose = vi.fn();
  const modelDispose = vi.fn();
  const changeDispose = vi.fn();
  const createModel = vi.fn((value: string) => ({
    dispose: modelDispose,
    getValue: vi.fn(() => value),
    onDidChangeContent: vi.fn(() => ({ dispose: changeDispose })),
    setValue: vi.fn(),
  }));
  const create = vi.fn(() => ({
    addCommand: vi.fn(),
    dispose: editorDispose,
    focus: vi.fn(),
    getValue: vi.fn(() => ""),
    layout: vi.fn(),
    trigger: vi.fn(),
    updateOptions: vi.fn(),
  }));

  return {
    changeDispose,
    create,
    createModel,
    editorDispose,
    modelDispose,
  };
});

vi.mock("monaco-editor", () => ({
  editor: {
    create: monacoMock.create,
    createModel: monacoMock.createModel,
  },
  KeyCode: { KeyS: 2 },
  KeyMod: { CtrlCmd: 1 },
  languages: {
    getLanguages: vi.fn(() => []),
    register: vi.fn(),
    setMonarchTokensProvider: vi.fn(),
  },
  Uri: { parse: vi.fn((value: string) => value) },
}));

const editorState: WorkspaceFileEditorState = {
  content: "# Notes\n\nHello",
  error: null,
  isDirty: false,
  isLoading: false,
  isSaving: false,
  lastSavedContent: "# Notes\n\nHello",
};

describe("WorkspaceFileEditorPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("disposes Monaco while markdown preview is active and recreates it for editing", async () => {
    render(
      <WorkspaceFileEditorPanel
        editor={editorState}
        file={{
          name: "README.md",
          path: "README.md",
          workspaceId: "workspace-1",
          workspaceLogoUrl: null,
          workspaceName: "Default",
        }}
        onChangeContent={vi.fn()}
        onReload={vi.fn(async () => undefined)}
        onSave={vi.fn(async () => true)}
      />,
    );

    await waitFor(() => expect(monacoMock.create).toHaveBeenCalledTimes(1));

    await userEvent.click(screen.getByRole("button", { name: "Preview markdown" }));

    const editMarkdownButton = screen.getByRole("button", { name: "Edit markdown" });
    expect(editMarkdownButton).toHaveAttribute("aria-pressed", "true");
    expect(editMarkdownButton).toHaveClass("button--tertiary");

    expect(monacoMock.editorDispose).toHaveBeenCalledTimes(1);
    expect(monacoMock.modelDispose).toHaveBeenCalledTimes(1);

    await userEvent.click(editMarkdownButton);

    await waitFor(() => expect(monacoMock.create).toHaveBeenCalledTimes(2));
  });

  it("keeps the toolbar ordered, accessible, and neutral across active states", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();

    render(
      <WorkspaceFileEditorPanel
        editor={{ ...editorState, isDirty: true }}
        file={{
          name: "README.md",
          path: "README.md",
          workspaceId: "workspace-1",
          workspaceLogoUrl: null,
          workspaceName: "Default",
        }}
        onChangeContent={vi.fn()}
        onReload={vi.fn(async () => undefined)}
        onSave={onSave}
      />,
    );

    await waitFor(() => expect(monacoMock.create).toHaveBeenCalledTimes(1));

    const toolbar = screen.getByRole("toolbar", { name: "Editor toolbar" });
    const buttons = within(toolbar).getAllByRole("button");
    expect(buttons.map((button) => button.getAttribute("aria-label"))).toEqual([
      "Reload file",
      "Save",
      "Cut",
      "Copy",
      "Paste",
      "Undo",
      "Redo",
      "Find",
      "Word wrap",
      "Preview markdown",
    ]);

    const reloadButton = within(toolbar).getByRole("button", { name: "Reload file" });
    const saveButton = within(toolbar).getByRole("button", { name: "Save" });
    const wordWrapButton = within(toolbar).getByRole("button", { name: "Word wrap" });

    expect(reloadButton).toHaveClass(
      "workspace-file-editor-toolbar-button",
      "size-7",
      "min-w-7",
      "button--ghost",
      "button--icon-only",
      "button--sm",
    );
    expect(saveButton).toHaveAttribute("aria-pressed", "true");
    expect(saveButton).toHaveClass("button--tertiary");

    await user.click(saveButton);
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ path: "README.md" }), "");

    await user.click(wordWrapButton);
    expect(wordWrapButton).toHaveAttribute("aria-pressed", "true");
    expect(wordWrapButton).toHaveClass("button--tertiary");
  });
});
