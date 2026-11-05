import { authFetch } from "../authFetch";
import {
  beginApiRequest,
  finishApiRequest,
  summarizeFormData,
} from "../../lib/activityLog";

export const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL ?? "").replace(
  /\/$/,
  "",
);

validateApiBaseUrl(API_BASE_URL);

export class BackendApiError extends Error {
  status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "BackendApiError";
    this.status = status;
  }
}

export function apiUrl(path: string) {
  if (
    import.meta.env.MODE === "desktop" &&
    !import.meta.env.DEV &&
    !API_BASE_URL
  ) {
    throw new Error(
      "VITE_API_BASE_URL is required for packaged desktop builds.",
    );
  }

  return `${API_BASE_URL}${path}`;
}

function validateApiBaseUrl(baseUrl: string) {
  if (!baseUrl) {
    return;
  }

  let parsed: URL;
  try {
    parsed = new URL(baseUrl);
  } catch {
    throw new Error("VITE_API_BASE_URL must be an absolute URL.");
  }
  const localDevelopmentUrl =
    import.meta.env.DEV &&
    parsed.protocol === "http:" &&
    (parsed.hostname === "localhost" || parsed.hostname === "127.0.0.1");
  if (parsed.protocol !== "https:" && !localDevelopmentUrl) {
    throw new Error(
      "VITE_API_BASE_URL must use HTTPS outside local development.",
    );
  }
}

export async function requestJson<TResponse>(
  path: string,
  init?: RequestInit,
  requestDetails?: unknown,
  logResponseBody = true,
) {
  const url = apiUrl(path);
  const method = init?.method ?? "GET";
  const requestId = beginApiRequest({
    method,
    request: requestDetails,
    url: logResponseBody ? url : url.split("?")[0],
  });
  const startedAt = performance.now();

  try {
    const response = await authFetch(url, init);
    const bodyText = response.status === 204 ? "" : await response.text();
    let body: unknown;

    if (bodyText) {
      try {
        body = JSON.parse(bodyText);
      } catch {
        body = bodyText;
      }
    }

    if (!response.ok) {
      const errorBody = body as
        | { error?: string; message?: string }
        | undefined;
      const message =
        (typeof errorBody === "object"
          ? (errorBody?.error ?? errorBody?.message)
          : bodyText) ||
        response.statusText ||
        `Request failed with status ${response.status}`;

      finishApiRequest(requestId, {
        details: logResponseBody ? body : undefined,
        durationMs: performance.now() - startedAt,
        httpStatus: response.status,
        message,
        status: "error",
      });
      throw new BackendApiError(message, response.status);
    }

    finishApiRequest(requestId, {
      details: logResponseBody ? body : undefined,
      durationMs: performance.now() - startedAt,
      httpStatus: response.status,
      status: "success",
    });
    return body as TResponse;
  } catch (error) {
    if (!(error instanceof BackendApiError)) {
      finishApiRequest(requestId, {
        details: error,
        durationMs: performance.now() - startedAt,
        message:
          error instanceof Error ? error.message : "Network request failed",
        status: "error",
      });
    }
    throw error;
  }
}

export async function get<TResponse>(path: string): Promise<TResponse> {
  return requestJson<TResponse>(path);
}

export async function post<TResponse, TPayload>(
  path: string,
  payload: TPayload,
): Promise<TResponse> {
  return requestJson<TResponse>(
    path,
    {
      body: JSON.stringify(payload),
      headers: { "Content-Type": "application/json" },
      method: "POST",
    },
    payload,
  );
}

export async function postForm<TResponse>(
  path: string,
  formData: FormData,
): Promise<TResponse> {
  return requestJson<TResponse>(
    path,
    {
      body: formData,
      method: "POST",
    },
    summarizeFormData(formData),
  );
}

export function appendFiles(form: FormData, files: File[]) {
  for (const file of files) {
    const relativeFile = file as File & { webkitRelativePath?: string };
    form.append("files", file, relativeFile.webkitRelativePath || file.name);
  }
}

/** JSON requests whose payloads and responses must stay out of activity details. */
export function requestPrivateJson<T>(
  method: "GET" | "POST" | "PATCH" | "DELETE",
  path: string,
  body?: unknown,
) {
  return requestJson<T>(
    path,
    {
      method,
      ...(body === undefined
        ? {}
        : {
            body: JSON.stringify(body),
            headers: { "Content-Type": "application/json" },
          }),
    },
    undefined,
    false,
  );
}
