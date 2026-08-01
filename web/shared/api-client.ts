export type ApiDiagnostic = {
  diagnosticId: string;
  operation: string | null;
  phase: string | null;
};

export class ApiRequestError extends Error {
  readonly diagnostic: ApiDiagnostic | null;

  constructor(message: string, diagnostic: ApiDiagnostic | null = null) {
    super(message);
    this.name = "ApiRequestError";
    this.diagnostic = diagnostic;
  }
}

export async function responseError(response: Response): Promise<ApiRequestError> {
  const contentType = response.headers.get("content-type") ?? "";
  let message = `request returned ${response.status}`;
  let diagnostic: ApiDiagnostic | null = diagnosticFromHeaders(response.headers);

  if (contentType.includes("application/json")) {
    const data = (await response.json()) as unknown;

    if (isErrorResponse(data)) {
      message = data.error;
      diagnostic = diagnosticFromErrorResponse(data) ?? diagnostic;
    }
  } else {
    const text = await response.text();
    message = text || message;
  }

  return new ApiRequestError(message, diagnostic);
}

export async function responseErrorMessage(response: Response) {
  return (await responseError(response)).message;
}

export async function requestJson<T>(
  url: string,
  init?: RequestInit,
): Promise<T> {
  const response = await fetch(url, {
    cache: "no-store",
    credentials: "same-origin",
    ...init,
  });
  const contentType = response.headers.get("content-type") ?? "";
  const data = contentType.includes("application/json")
    ? ((await response.json()) as unknown)
    : null;

  if (!response.ok) {
    throw await responseErrorFromData(response, data, url);
  }

  return data as T;
}

async function responseErrorFromData(
  response: Response,
  data: unknown,
  url: string,
) {
  if (isErrorResponse(data)) {
    return new ApiRequestError(
      data.error,
      diagnosticFromErrorResponse(data) ?? diagnosticFromHeaders(response.headers),
    );
  }

  return new ApiRequestError(
    `${url} returned ${response.status}`,
    diagnosticFromHeaders(response.headers),
  );
}

function isErrorResponse(value: unknown): value is {
  error: string;
  diagnostic?: unknown;
} {
  return (
    typeof value === "object" &&
    value !== null &&
    "error" in value &&
    typeof value.error === "string"
  );
}

export function errorMessage(value: unknown) {
  return value instanceof Error ? value.message : "Unknown error";
}

export function errorDiagnostic(value: unknown): ApiDiagnostic | null {
  return value instanceof ApiRequestError ? value.diagnostic : null;
}

function diagnosticFromErrorResponse(value: { diagnostic?: unknown }) {
  return diagnosticFromUnknown(value.diagnostic);
}

function diagnosticFromHeaders(headers: Headers): ApiDiagnostic | null {
  return diagnosticFromUnknown({
    diagnosticId: headers.get("x-foco-chat-not-found-diagnostic-id"),
    operation: headers.get("x-foco-chat-not-found-operation"),
    phase: headers.get("x-foco-chat-not-found-phase"),
  });
}

function diagnosticFromUnknown(value: unknown): ApiDiagnostic | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }

  const record = value as Record<string, unknown>;
  const diagnosticId = record.diagnosticId;
  if (typeof diagnosticId !== "string" || !diagnosticId.trim()) {
    return null;
  }

  return {
    diagnosticId,
    operation:
      typeof record.operation === "string" && record.operation.trim()
        ? record.operation
        : null,
    phase:
      typeof record.phase === "string" && record.phase.trim()
        ? record.phase
        : null,
  };
}
