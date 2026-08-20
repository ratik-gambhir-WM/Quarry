import type {
  AuthenticationResult,
  InteractionMode,
  MsalInstance,
  MsalTokenProviderConfig,
  RefreshTokensResult,
} from "./types";

const DEFAULT_GRAPH_SCOPES = ["Sites.Read.All"];
const DEFAULT_INTERACTION_MODES: InteractionMode[] = ["popup", "redirect"];

/**
 * Browser-side helper for acquiring Azure AD / Microsoft Graph tokens
 * using an MSAL `PublicClientApplication` (user-delegated flow).
 *
 * Wraps `acquireTokenSilent` with an optional fallback to `acquireTokenPopup`
 * and `acquireTokenRedirect`. The caller is responsible for persisting the
 * returned tokens (e.g. localStorage) — this class does not touch storage.
 *
 * Pair the acquired Graph token with a server-side
 * {@link import("../server").SharePointClient} by passing it as the
 * `graphToken` argument to any method.
 */
export class MsalTokenProvider {
  private readonly instance: MsalInstance;
  private readonly apiScopes: string[];
  private readonly graphScopes: string[];
  private readonly interactionModes: InteractionMode[];
  private readonly onAcquireFailed?: (error: unknown) => void | Promise<void>;

  constructor(config: MsalTokenProviderConfig) {
    this.instance = config.instance;
    this.apiScopes = config.apiScopes;
    this.graphScopes = config.graphScopes ?? DEFAULT_GRAPH_SCOPES;
    this.interactionModes = normalizeInteractionModes(config.interactionMode);
    this.onAcquireFailed = config.onAcquireFailed;
  }

  /** Acquire a token for the configured API scopes. */
  getApiToken(): Promise<AuthenticationResult> {
    return this.acquire(this.apiScopes);
  }

  /** Acquire a Microsoft Graph token for the configured Graph scopes. */
  getGraphToken(): Promise<AuthenticationResult> {
    return this.acquire(this.graphScopes);
  }

  /**
   * Acquire both API and Graph tokens. Returns the raw MSAL
   * `AuthenticationResult` objects for each — callers can persist
   * `result.apiToken.accessToken` / `result.graphToken.accessToken` however
   * they see fit.
   */
  async refreshTokens(): Promise<RefreshTokensResult> {
    const apiToken = await this.getApiToken();
    const graphToken = await this.getGraphToken();
    return { apiToken, graphToken };
  }

  /** Internal: acquire a token for a specific scope set with the configured fallbacks. */
  private async acquire(scopes: string[]): Promise<AuthenticationResult> {
    const account = this.instance.getAllAccounts()[0];
    if (!account) {
      const err = new Error(
        "No active MSAL account available for token acquisition.",
      );
      await this.onAcquireFailed?.(err);
      throw err;
    }

    let lastError: unknown;
    try {
      return await this.instance.acquireTokenSilent({ scopes, account });
    } catch (silentError) {
      lastError = silentError;
    }

    for (const mode of this.interactionModes) {
      if (mode === "none") {
        await this.onAcquireFailed?.(lastError);
        throw lastError;
      }

      try {
        if (mode === "popup") {
          return await this.instance.acquireTokenPopup({ scopes, account });
        }
        // mode === "redirect"
        await this.instance.acquireTokenRedirect({ scopes, account });
        // acquireTokenRedirect navigates away; returning is a formality.
        return Promise.reject(
          new Error("Token acquisition redirected; page will reload."),
        );
      } catch (interactiveError) {
        lastError = interactiveError;
      }
    }

    await this.onAcquireFailed?.(lastError);
    throw lastError;
  }
}

function normalizeInteractionModes(
  mode: InteractionMode | InteractionMode[] | undefined,
): InteractionMode[] {
  if (mode === undefined) return [...DEFAULT_INTERACTION_MODES];
  return Array.isArray(mode) ? [...mode] : [mode];
}
