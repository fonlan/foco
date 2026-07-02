import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

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

    expect(monacoMock.editorDispose).toHaveBeenCalledTimes(1);
    expect(monacoMock.modelDispose).toHaveBeenCalledTimes(1);

    await userEvent.click(screen.getByRole("button", { name: "Edit markdown" }));

    await waitFor(() => expect(monacoMock.create).toHaveBeenCalledTimes(2));
  });
});
