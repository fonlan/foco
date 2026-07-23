import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { useDocumentTheme } from "../app/app-effects";

function resetDocumentTheme() {
  document.documentElement.classList.remove("light", "dark");
  document.documentElement.removeAttribute("data-theme");
  document.documentElement.removeAttribute("data-foco-theme");
  document.documentElement.style.colorScheme = "";
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) {
    meta.setAttribute("content", "#f7f7f7");
  }
}

describe("useDocumentTheme", () => {
  afterEach(() => {
    resetDocumentTheme();
  });

  it("applies light HeroUI theme contract on mount", () => {
    if (!document.querySelector('meta[name="theme-color"]')) {
      const meta = document.createElement("meta");
      meta.setAttribute("name", "theme-color");
      meta.setAttribute("content", "#000000");
      document.head.appendChild(meta);
    }

    renderHook(() => useDocumentTheme("light"));

    expect(document.documentElement.classList.contains("light")).toBe(true);
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.style.colorScheme).toBe("light");
    expect(
      document.querySelector('meta[name="theme-color"]')?.getAttribute("content"),
    ).toBe("#f7f7f7");
  });

  it("switches to dark and back at runtime", () => {
    if (!document.querySelector('meta[name="theme-color"]')) {
      const meta = document.createElement("meta");
      meta.setAttribute("name", "theme-color");
      document.head.appendChild(meta);
    }

    const { rerender } = renderHook(
      ({ theme }: { theme: string }) => useDocumentTheme(theme),
      { initialProps: { theme: "light" } },
    );

    expect(document.documentElement.dataset.theme).toBe("light");

    rerender({ theme: "dark" });
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.classList.contains("light")).toBe(false);
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
    expect(
      document.querySelector('meta[name="theme-color"]')?.getAttribute("content"),
    ).toBe("#1c1b1f");

    rerender({ theme: "light" });
    expect(document.documentElement.classList.contains("light")).toBe(true);
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.documentElement.style.colorScheme).toBe("light");
  });
});
