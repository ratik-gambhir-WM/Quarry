import { SharePointClientError } from "./errors";
import type { UserGraphProxyConfig } from "../types";

export const GRAPH_BASE_URL = "https://graph.microsoft.com/v1.0";

export interface GraphRequestContext {
  token?: string;
  userGraphProxy?: UserGraphProxyConfig;
}

interface GraphRequestOptions {
  requiresUserContext?: boolean;
}

/** True for Graph paths that are explicitly scoped to a user. */
export function isUserScopedGraphPath(pathOrUrl: string): boolean {
  const path = normalizeGraphPath(pathOrUrl);
  const pathname = splitPathAndSuffix(path).pathname;
  return (
    pathname === "/me" ||
    pathname.startsWith("/me/") ||
    /^\/users\/[^/]+(?:\/|$)/.test(pathname)
  );
}

/** Build a Graph URL, using the user proxy only for user-context requests. */
export function buildGraphUrl(
  pathOrUrl: string,
  context: GraphRequestContext = {},
  options: GraphRequestOptions = {},
): string {
  const path = normalizeGraphPath(pathOrUrl);
  if (shouldUseUserGraphProxy(path, context, options)) {
    return joinUrl(
      context.userGraphProxy?.baseUrl ?? "",
      buildUserGraphProxyPath(path, context.userGraphProxy),
    );
  }
  return joinUrl(GRAPH_BASE_URL, path);
}

/** Build fetch input for a Graph call, omitting Authorization for proxy calls. */
export function buildGraphRequest(
  pathOrUrl: string,
  context: GraphRequestContext,
  init: RequestInit = {},
  options: GraphRequestOptions = {},
): { url: string; init: RequestInit } {
  const path = normalizeGraphPath(pathOrUrl);
  const useProxy = shouldUseUserGraphProxy(path, context, options);

  if (!useProxy && !context.token) {
    throw new SharePointClientError(
      "A Graph token is required for direct Microsoft Graph requests.",
    );
  }

  const headers = {
    ...((init.headers as Record<string, string> | undefined) ?? {}),
  };

  if (useProxy) {
    delete headers.Authorization;
    delete headers.authorization;
  } else {
    headers.Authorization = context.token ?? "";
  }

  return {
    url: buildGraphUrl(path, context, options),
    init: { ...init, headers },
  };
}

export function shouldUseUserGraphProxy(
  pathOrUrl: string,
  context: GraphRequestContext,
  options: GraphRequestOptions = {},
): boolean {
  return Boolean(
    context.userGraphProxy &&
      (options.requiresUserContext || isUserScopedGraphPath(pathOrUrl)),
  );
}

function buildUserGraphProxyPath(
  path: string,
  proxy?: UserGraphProxyConfig,
): string {
  const { pathname, suffix } = splitPathAndSuffix(path);
  const defaultUserId = encodeURIComponent(proxy?.userId ?? "me");

  if (pathname === "/me" || pathname.startsWith("/me/")) {
    const tail = pathname.slice("/me".length);
    return `/users/${defaultUserId}/graph${tail}${suffix}`;
  }

  const usersMatch = pathname.match(/^\/users\/([^/]+)(\/.*)?$/);
  if (usersMatch) {
    const userId = usersMatch[1];
    const tail = usersMatch[2] ?? "";
    return `/users/${userId}/graph${tail}${suffix}`;
  }

  return `/users/${defaultUserId}/graph${pathname}${suffix}`;
}

function normalizeGraphPath(pathOrUrl: string): string {
  if (pathOrUrl.startsWith(GRAPH_BASE_URL)) {
    return pathOrUrl.slice(GRAPH_BASE_URL.length) || "/";
  }
  return pathOrUrl.startsWith("/") ? pathOrUrl : `/${pathOrUrl}`;
}

function splitPathAndSuffix(path: string): { pathname: string; suffix: string } {
  const suffixStart = path.search(/[?#]/);
  if (suffixStart === -1) {
    return { pathname: path, suffix: "" };
  }
  return {
    pathname: path.slice(0, suffixStart),
    suffix: path.slice(suffixStart),
  };
}

function joinUrl(baseUrl: string, path: string): string {
  return `${baseUrl.replace(/\/+$/g, "")}/${path.replace(/^\/+/g, "")}`;
}
