import { SharePointClientError } from "./errors";
import type {
  CacheAdapter,
  SharePointClientConfig,
  TokenResponse,
} from "../types";
import { hashToken } from "./utils";

const TOKEN_CACHE_KEY_PREFIX = "sharepoint-client:oauth-token";
/** Refresh the token 60 seconds before it expires. */
const EXPIRY_SAFETY_MARGIN_MS = 60_000;

/** Internal token manager — auto-acquires and caches client_credentials tokens. */
export class TokenManager {
  private readonly tenantId?: string;
  private readonly clientId?: string;
  private readonly clientSecret?: string;
  private readonly cache: CacheAdapter;

  constructor(config: SharePointClientConfig, cache: CacheAdapter) {
    this.tenantId = config.tenantId;
    this.clientId = config.clientId;
    this.clientSecret = config.clientSecret;
    this.cache = cache;
  }

  /**
   * Returns a cache key scoped to this TokenManager's credentials so that
   * multiple instances sharing the same cache backend cannot read each
   * other's tokens. The secret is hashed (never stored in the key).
   */
  private async getCacheKey(): Promise<string> {
    const credentials = this.getCredentials();

    const credentialKey = [
      credentials.tenantId,
      credentials.clientId,
      credentials.clientSecret,
    ].join(":");
    const hash = await hashToken(credentialKey);

    return `${TOKEN_CACHE_KEY_PREFIX}:${hash}`;
  }

  /** Returns a valid Graph API token, refreshing if needed. */
  async getToken(): Promise<string> {
    const credentials = this.getCredentials();

    const cacheKey = await this.getCacheKey();
    const cached = await this.cache.get<string>(cacheKey);
    if (cached) return cached;

    const response = await fetch(
      `https://login.microsoftonline.com/${credentials.tenantId}/oauth2/v2.0/token`,
      {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({
          client_id: credentials.clientId,
          scope: "https://graph.microsoft.com/.default",
          client_secret: credentials.clientSecret,
          grant_type: "client_credentials",
        }),
      },
    );

    if (!response.ok) {
      const body = await response.text();
      throw new SharePointClientError(
        `Failed to acquire Graph token: ${response.status}`,
        response.status,
        body,
      );
    }

    const data = (await response.json()) as TokenResponse;

    if (!data.access_token) {
      throw new SharePointClientError(
        "Token response missing access_token",
        500,
        data,
      );
    }

    const ttlMs = data.expires_in * 1_000 - EXPIRY_SAFETY_MARGIN_MS;
    await this.cache.set(cacheKey, data.access_token, Math.max(ttlMs, 0));

    return data.access_token;
  }

  private getCredentials(): {
    tenantId: string;
    clientId: string;
    clientSecret: string;
  } {
    if (!this.tenantId || !this.clientId || !this.clientSecret) {
      throw new SharePointClientError(
        "tenantId, clientId, and clientSecret are required for direct Microsoft Graph token acquisition.",
        400,
      );
    }

    return {
      tenantId: this.tenantId,
      clientId: this.clientId,
      clientSecret: this.clientSecret,
    };
  }
}
