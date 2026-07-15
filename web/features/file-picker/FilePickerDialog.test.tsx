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
    allowOutsideWorkspace?: boolean;
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
    const body = listBodyAt(fetchMock.mock.calls.indexOf(listCall!));
    expect(body.target).toEqual({
      kind: "workspace",
      workspaceId: "workspace-1",
    });
    expect(body.allowOutsideWorkspace).toBe(false);
  });

  it("forwards allowOutsideWorkspace on list and read-files when enabled", async () => {
    const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
      const url = typeof input === "string" ? input : input.toString();
      if (url === "/api/file-picker/list") {
        return jsonResponse({
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
        });
      }
      if (url === "/api/file-picker/read-files") {
        return jsonResponse({
          files: [
            {
              contentBase64: "SGVsbG8=",
              contentType: "text/plain",
              name: "note.txt",
              path: "/Users/me/note.txt",
              sizeBytes: 5,
            },
          ],
        });
      }
      return jsonResponse({});
    });

    const onSelect = vi.fn();
    render(
      <FilePickerDialog
        allowOutsideWorkspace={true}
        mode="file"
        open={true}
        readFiles={true}
        target={{ kind: "workspace", workspaceId: "workspace-1" }}
        title="Add attachment"
        t={(key) => key}
        onClose={() => undefined}
        onSelect={onSelect}
      />,
    );

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) => typeof url === "string" && url === "/api/file-picker/list",
        ),
      ).toBe(true);
    });
    const listCall = fetchMock.mock.calls.find(
      ([url]) => typeof url === "string" && url === "/api/file-picker/list",
    );
    expect(listCall).toBeDefined();
    expect(listBodyAt(fetchMock.mock.calls.indexOf(listCall!)).allowOutsideWorkspace).toBe(true);

    await userEvent.click(screen.getByRole("button", { name: /note\.txt/ }));
    await userEvent.click(screen.getByRole("button", { name: "Select" }));

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some(
          ([url]) => typeof url === "string" && url === "/api/file-picker/read-files",
        ),
      ).toBe(true);
    });
    const readCall = fetchMock.mock.calls.find(
      ([url]) => typeof url === "string" && url === "/api/file-picker/read-files",
    );
    const readBody = JSON.parse(String(readCall?.[1]?.body ?? "{}")) as {
      allowOutsideWorkspace?: boolean;
      paths?: string[];
      target?: { kind?: string; workspaceId?: string };
    };
    expect(readBody.allowOutsideWorkspace).toBe(true);
    expect(readBody.paths).toEqual(["/Users/me/note.txt"]);
    expect(readBody.target).toEqual({
      kind: "workspace",
      workspaceId: "workspace-1",
    });
    expect(onSelect).toHaveBeenCalled();
  });
});
