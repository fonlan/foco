import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FilePickerDialog } from "./FilePickerDialog";

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    headers: { "Content-Type": "application/json" },
    status: 200,
  });
}

function listBodyAt(index: number) {
  const body = vi.mocked(fetch).mock.calls[index]?.[1]?.body;
  return JSON.parse(String(body ?? "{}")) as {
    showHidden?: boolean;
    target?: { kind?: string; workspaceId?: string; serverId?: string };
  };
}

describe("FilePickerDialog", () => {
  it("sends showHidden false by default and true after toggling hidden files", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({
        entries: [
          {
            disabled: false,
            isDirectory: false,
            modifiedMs: null,
            name: "note.txt",
            path: "/Users/me/note.txt",
            sizeBytes: 5,
          },
        ],
        parentPath: "/Users",
        path: "/Users/me",
        truncated: false,
        warnings: [],
      }),
    );

    render(
      <FilePickerDialog
        initialPath="/Users/me"
        mode="file"
        open={true}
        target={{ kind: "local" }}
        title="Select file"
        t={(key) => key}
        onClose={() => undefined}
        onSelect={() => undefined}
      />,
    );

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    expect(listBodyAt(0).showHidden).toBe(false);

    await userEvent.click(screen.getByLabelText("Show hidden files"));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
    expect(listBodyAt(1).showHidden).toBe(true);
  });

  it("posts workspace target with camelCase workspaceId to list", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
      jsonResponse({
        entries: [],
        parentPath: null,
        path: "/workspace",
        truncated: false,
        warnings: [],
      }),
    );

    render(
      <FilePickerDialog
        mode="file"
        open={true}
        target={{ kind: "workspace", workspaceId: "workspace-1" }}
        title="Add attachment"
        t={(key) => key}
        onClose={() => undefined}
        onSelect={() => undefined}
      />,
    );

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    const listCall = fetchMock.mock.calls.find(
      ([url]) => typeof url === "string" && url === "/api/file-picker/list",
    );
    expect(listCall).toBeDefined();
    expect(listBodyAt(fetchMock.mock.calls.indexOf(listCall!)).target).toEqual({
      kind: "workspace",
      workspaceId: "workspace-1",
    });
  });
});
