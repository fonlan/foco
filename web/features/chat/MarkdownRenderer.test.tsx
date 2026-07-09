import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { MarkdownRenderer } from "./MarkdownRenderer";

describe("MarkdownRenderer", () => {
  it("opens markdown links in a new browser tab", () => {
    render(<MarkdownRenderer allowHtml={false} content="[docs](https://example.com)" />);

    const link = screen.getByRole("link", { name: "docs" });

    expect(link).toHaveAttribute("href", "https://example.com");
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noreferrer");
  });

  it("copies fenced code block text", async () => {
    const writeText = vi.mocked(navigator.clipboard.writeText);
    writeText.mockClear();
    writeText.mockResolvedValue(undefined);

    render(
      <MarkdownRenderer
        allowHtml={false}
        content={"```ts\nconst answer = 42;\n```"}
      />,
    );

    const copyButton = screen.getByRole("button", { name: "Copy code" });

    expect(copyButton).toHaveTextContent("");
    await userEvent.click(copyButton);

    expect(writeText).toHaveBeenCalledWith("const answer = 42;\n");
    expect(
      await screen.findByRole("button", { name: "Copied code" }),
    ).toBeInTheDocument();
  });
});
