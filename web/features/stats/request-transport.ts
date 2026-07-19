import type { AiRequestTransport, Translate } from "../../api/types";

/** Normalize API/wire transport to the stable audit union. Never infers from Provider config. */
export function normalizeRequestTransport(
  transport: AiRequestTransport | string | null | undefined,
): AiRequestTransport {
  if (transport === "http" || transport === "websocket" || transport === "unknown") {
    return transport;
  }
  return "unknown";
}

/** Localized transport label for list suffix and detail overview. */
export function requestTransportLabel(
  transport: AiRequestTransport | string | null | undefined,
  t: Translate,
): string {
  switch (normalizeRequestTransport(transport)) {
    case "http":
      return t("HTTP");
    case "websocket":
      return t("WebSocket");
    case "unknown":
      return t("Unknown transport");
  }
}

/**
 * Parenthetical protocol suffix for the provider row only.
 * English: `(HTTP)`; Chinese: `（HTTP）` via i18n template.
 */
export function formatProviderTransportSuffix(
  transport: AiRequestTransport | string | null | undefined,
  t: Translate,
): string {
  return t("({transport})", {
    transport: requestTransportLabel(transport, t),
  });
}
