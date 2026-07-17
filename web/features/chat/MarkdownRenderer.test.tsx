import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { MouseEvent as ReactMouseEvent } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  handleMarkdownAnchorClick,
  MarkdownRenderer,
} from "./MarkdownRenderer";

describe("MarkdownRenderer", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders markdown links with a safe new-tab target", () => {
    render(<MarkdownRenderer allowHtml={false} content="[docs](https://example.com)" />);

    const link = screen.getByRole("link", { name: "docs" });

    expect(link).toHaveAttribute("href", "https://example.com");
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("opens markdown links in a new browser tab on plain primary click", async () => {
    const openSpy = vi.spyOn(window, "open").mockReturnValue(null);

    render(<MarkdownRenderer allowHtml={false} content="[docs](https://example.com/path)" />);

    const link = screen.getByRole("link", { name: "docs" });
    await userEvent.click(link);

    expect(openSpy).toHaveBeenCalledTimes(1);
    expect(openSpy).toHaveBeenCalledWith(
      "https://example.com/path",
      "_blank",
      "noopener,noreferrer",
    );
  });

  it("opens markdown links in a new browser tab on keyboard Enter activation", async () => {
    const openSpy = vi.spyOn(window, "open").mockReturnValue(null);
    const user = userEvent.setup();

    render(
      <MarkdownRenderer allowHtml={false} content="[docs](https://example.com/keyboard)" />,
    );

    const link = screen.getByRole("link", { name: "docs" });
    link.focus();
    expect(link).toHaveFocus();
    await user.keyboard("{Enter}");

    expect(openSpy).toHaveBeenCalledTimes(1);
    expect(openSpy).toHaveBeenCalledWith(
      "https://example.com/keyboard",
      "_blank",
      "noopener,noreferrer",
    );
  });

  it("does not call window.open when upstream onClick prevents default", () => {
    const openSpy = vi.spyOn(window, "open").mockReturnValue(null);
    let prevented = false;
    const event = {
      altKey: false,
      button: 0,
      ctrlKey: false,
      metaKey: false,
      preventDefault: () => {
        prevented = true;
      },
      shiftKey: false,
    } as unknown as ReactMouseEvent<HTMLAnchorElement>;
    Object.defineProperty(event, "defaultPrevented", {
      configurable: true,
      get() {
        return prevented;
      },
    });

    handleMarkdownAnchorClick(event, "https://example.com/blocked", (clickEvent) => {
      clickEvent.preventDefault();
    });

    expect(openSpy).not.toHaveBeenCalled();
  });

  it("does not call window.open when the link has no href", () => {
    const openSpy = vi.spyOn(window, "open").mockReturnValue(null);
    const preventDefault = vi.fn();
    const event = {
      altKey: false,
      button: 0,
      ctrlKey: false,
      defaultPrevented: false,
      metaKey: false,
      preventDefault,
      shiftKey: false,
    } as unknown as ReactMouseEvent<HTMLAnchorElement>;

    handleMarkdownAnchorClick(event, undefined);

    expect(openSpy).not.toHaveBeenCalled();
    expect(preventDefault).not.toHaveBeenCalled();
  });

  it("does not hijack modified primary clicks", async () => {
    const openSpy = vi.spyOn(window, "open").mockReturnValue(null);

    render(<MarkdownRenderer allowHtml={false} content="[docs](https://example.com/mod)" />);

    const link = screen.getByRole("link", { name: "docs" });
    fireEvent.click(link, { button: 0, ctrlKey: true });
    fireEvent.click(link, { button: 0, metaKey: true });
    fireEvent.click(link, { button: 0, shiftKey: true });
    fireEvent.click(link, { button: 0, altKey: true });

    expect(openSpy).not.toHaveBeenCalled();
  });

  it("does not hijack non-primary button clicks", () => {
    const openSpy = vi.spyOn(window, "open").mockReturnValue(null);
    const preventDefault = vi.fn();
    const event = {
      altKey: false,
      button: 1,
      ctrlKey: false,
      defaultPrevented: false,
      metaKey: false,
      preventDefault,
      shiftKey: false,
    } as unknown as ReactMouseEvent<HTMLAnchorElement>;

    handleMarkdownAnchorClick(event, "https://example.com/middle");

    expect(openSpy).not.toHaveBeenCalled();
    expect(preventDefault).not.toHaveBeenCalled();
  });

  it("copies fenced code block text", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        write: vi.fn().mockResolvedValue(undefined),
        writeText,
      },
    });

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
