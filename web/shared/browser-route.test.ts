import { describe, expect, it } from "vitest";

import {
  browserPathForRoute,
  browserRouteFromPathname,
} from "./browser-route";

describe("browser route chat tabs", () => {
  it("parses chat tabs from repeated query params and keeps the active chat", () => {
    expect(
      browserRouteFromPathname(
        "/workspace-1/chat-2",
        "?tab=workspace-1%2Fchat-1&tab=workspace-2%2Fside-chat-1",
      ),
    ).toEqual({
      chatId: "chat-2",
      tabs: [
        { chatId: "chat-1", workspaceId: "workspace-1" },
        { chatId: "side-chat-1", workspaceId: "workspace-2" },
        { chatId: "chat-2", workspaceId: "workspace-1" },
      ],
      viewMode: "chat",
      workspaceId: "workspace-1",
    });
  });

  it("serializes chat tabs into the route query string", () => {
    expect(
      browserPathForRoute({
        chatId: "chat-2",
        tabs: [
          { chatId: "chat-1", workspaceId: "workspace-1" },
          { chatId: "chat-2", workspaceId: "workspace-1" },
        ],
        viewMode: "chat",
        workspaceId: "workspace-1",
      }),
    ).toBe(
      "/workspace-1/chat-2?tab=workspace-1%2Fchat-1&tab=workspace-1%2Fchat-2",
    );
  });

  it("normalizes legacy chat routes into a restorable single-tab route", () => {
    expect(browserRouteFromPathname("/workspace-1/chat-1")).toEqual({
      chatId: "chat-1",
      tabs: [{ chatId: "chat-1", workspaceId: "workspace-1" }],
      viewMode: "chat",
      workspaceId: "workspace-1",
    });
  });

  it("round-trips the scheduled tasks route", () => {
    expect(browserRouteFromPathname("/scheduled")).toEqual({
      viewMode: "scheduled",
    });
    expect(browserPathForRoute({ viewMode: "scheduled" })).toBe("/scheduled");
  });

  it("parses and serializes stats page numbers", () => {
    expect(browserRouteFromPathname("/stats", "?page=3")).toEqual({
      page: 3,
      viewMode: "stats",
    });
    expect(browserPathForRoute({ page: 3, viewMode: "stats" })).toBe(
      "/stats?page=3",
    );
  });

  it("falls back to stats page 1 for invalid page values", () => {
    for (const search of ["", "?page=0", "?page=-1", "?page=abc", "?page=1.5"]) {
      expect(browserRouteFromPathname("/stats", search)).toEqual({
        page: 1,
        viewMode: "stats",
      });
    }
  });
});
