import { useState } from "react";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { WorkspaceFileEditorPanel } from "./WorkspaceFileEditorPanel";
import type { WorkspaceFileEditorState } from "./WorkspaceFileEditorPanel";

const monacoMock = vi.hoisted(() => {
  const editorDispose = vi.fn();
  const modelDispose = vi.fn();
  const changeDispose = vi.fn();
  const editors: Array<{
    addCommand: ReturnType<typeof vi.fn>;
    dispose: ReturnType<typeof vi.fn>;
    focus: ReturnType<typeof vi.fn>;
    getValue: ReturnType<typeof vi.fn>;
    layout: ReturnType<typeof vi.fn>;
    restoreViewState: ReturnType<typeof vi.fn>;
    saveViewState: ReturnType<typeof vi.fn>;
    trigger: ReturnType<typeof vi.fn>;
    updateOptions: ReturnType<typeof vi.fn>;
  }> = [];
  const createModel = vi.fn((value: string) => ({
    dispose: modelDispose,
    getValue: vi.fn(() => value),
    onDidChangeContent: vi.fn(() => ({ dispose: changeDispose })),
    setValue: vi.fn(),
  }));
  const create = vi.fn(() => {
    const viewState = { editorIndex: editors.length };
    const editor = {
      addCommand: vi.fn(),
      dispose: editorDispose,
      focus: vi.fn(),
      getValue: vi.fn(() => ""),
      layout: vi.fn(),
      restoreViewState: vi.fn(),
      saveViewState: vi.fn(() => viewState),
      trigger: vi.fn(),
      updateOptions: vi.fn(),
    };
    editors.push(editor);
    return editor;
  });

  return {
    changeDispose,
    create,
    createModel,
    editorDispose,
    editors,
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
  isMarkdownPreviewEnabled: false,
  isSaving: false,
  lastSavedContent: "# Notes\n\nHello",
};

describe("WorkspaceFileEditorPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    monacoMock.editors.length = 0;
  });

  it("disposes Monaco while controlled markdown preview is active and recreates it for editing", async () => {
    function ControlledPanel() {
      const [isMarkdownPreviewEnabled, setIsMarkdownPreviewEnabled] = useState(
        false,
      );

      return (
        <WorkspaceFileEditorPanel
          editor={{ ...editorState, isMarkdownPreviewEnabled }}
          file={{
            name: "README.md",
            path: "README.md",
            workspaceId: "workspace-1",
            workspaceLogoUrl: null,
            workspaceName: "Default",
          }}
          onChangeContent={vi.fn()}
          onMarkdownPreviewChange={(_, __, isEnabled) =>
            setIsMarkdownPreviewEnabled(isEnabled)
          }
          onReload={vi.fn(async () => undefined)}
          onRestoreViewState={vi.fn(() => null)}
          onSave={vi.fn(async () => true)}
          onSaveViewState={vi.fn()}
        />
      );
    }

    render(<ControlledPanel />);

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
        onMarkdownPreviewChange={vi.fn()}
        onReload={vi.fn(async () => undefined)}
        onRestoreViewState={vi.fn(() => null)}
        onSave={onSave}
        onSaveViewState={vi.fn()}
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

  it("saves and restores view state independently for each file tab", async () => {
    const viewStates = new Map<string, object>();
    const onRestoreViewState = vi.fn((workspaceId: string, path: string) =>
      viewStates.get(`${workspaceId}:${path}`) ?? null,
    );
    const onSaveViewState = vi.fn(
      (workspaceId: string, path: string, viewState: object) => {
        viewStates.set(`${workspaceId}:${path}`, viewState);
      },
    );
    const props = {
      editor: editorState,
      onChangeContent: vi.fn(),
      onMarkdownPreviewChange: vi.fn(),
      onReload: vi.fn(async () => undefined),
      onRestoreViewState,
      onSave: vi.fn(async () => true),
      onSaveViewState,
    };
    const file = (path: string) => ({
      name: path,
      path,
      workspaceId: "workspace-1",
      workspaceLogoUrl: null,
      workspaceName: "Default",
    });
    const { rerender } = render(
      <WorkspaceFileEditorPanel {...props} file={file("a.ts")} />,
    );

    await waitFor(() => expect(monacoMock.create).toHaveBeenCalledTimes(1));
    const firstEditor = monacoMock.editors[0];
    expect(firstEditor.restoreViewState).not.toHaveBeenCalled();

    rerender(<WorkspaceFileEditorPanel {...props} file={file("b.ts")} />);
    await waitFor(() => expect(monacoMock.create).toHaveBeenCalledTimes(2));
    const secondEditor = monacoMock.editors[1];
    expect(firstEditor.saveViewState).toHaveBeenCalledTimes(1);
    expect(secondEditor.restoreViewState).not.toHaveBeenCalled();

    rerender(<WorkspaceFileEditorPanel {...props} file={file("a.ts")} />);
    await waitFor(() => expect(monacoMock.create).toHaveBeenCalledTimes(3));
    const thirdEditor = monacoMock.editors[2];
    expect(secondEditor.saveViewState).toHaveBeenCalledTimes(1);
    expect(thirdEditor.restoreViewState).toHaveBeenCalledWith(
      firstEditor.saveViewState.mock.results[0]?.value,
    );
    expect(onSaveViewState).toHaveBeenCalledWith(
      "workspace-1",
      "a.ts",
      firstEditor.saveViewState.mock.results[0]?.value,
    );
  });
});
