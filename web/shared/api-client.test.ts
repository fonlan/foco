import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ApiRequestError,
  errorDiagnostic,
  requestJson,
} from "./api-client";

async function rejectedRequest() {
  try {
    await requestJson("/api/test");
    throw new Error("Expected requestJson to reject");
  } catch (error) {
    return error;
  }
}

describe("API error diagnostics", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("preserves the safe diagnostic from a local JSON error envelope", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            databaseIdentity: "must-not-be-exposed-to-the-browser-ui",
            diagnostic: {
              diagnosticId: "diagnostic-local-1",
              operation: "chat.context_usage",
              phase: "prompt-assembly",
            },
            error: "Chat is unavailable.",
          }),
          {
            headers: { "Content-Type": "application/json" },
            status: 404,
          },
        ),
      ),
    );

    const error = await rejectedRequest();

    expect(error).toBeInstanceOf(ApiRequestError);
    expect(errorDiagnostic(error)).toEqual({
      diagnosticId: "diagnostic-local-1",
      operation: "chat.context_usage",
      phase: "prompt-assembly",
    });
    expect((error as Error).message).toBe("Chat is unavailable.");
  });

  it("normalizes remote proxy headers to the same diagnostic shape", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ error: "Chat is unavailable." }), {
          headers: {
            "Content-Type": "application/json",
            "x-foco-chat-not-found-diagnostic-id": "diagnostic-remote-1",
            "x-foco-chat-not-found-operation": "chat.stream",
            "x-foco-chat-not-found-phase": "preflight-chat-lookup",
          },
          status: 404,
        }),
      ),
    );

    expect(errorDiagnostic(await rejectedRequest())).toEqual({
      diagnosticId: "diagnostic-remote-1",
      operation: "chat.stream",
      phase: "preflight-chat-lookup",
    });
  });

  it("keeps legacy and network errors compatible without a diagnostic", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ error: "Legacy failure" }), {
          headers: { "Content-Type": "application/json" },
          status: 500,
        }),
      ),
    );

    const error = await rejectedRequest();

    expect((error as Error).message).toBe("Legacy failure");
    expect(errorDiagnostic(error)).toBeNull();
  });
});
