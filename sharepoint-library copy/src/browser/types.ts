/**
 * Minimal shape of an MSAL browser `PublicClientApplication` that this library
 * depends on. Declaring it structurally (rather than importing
 * `IPublicClientApplication` from `@azure/msal-browser`) avoids binding consumers
 * to a specific copy of the MSAL types and sidesteps duplicate-type issues in
 * monorepos.
 */
export interface MsalAccountInfo {
  homeAccountId?: string;
  environment?: string;
  tenantId?: string;
  username?: string;
  localAccountId?: string;
}

export interface AuthenticationResult {
  accessToken: string;
  expiresOn?: Date | null;
  account?: MsalAccountInfo | null;
}

export interface MsalTokenRequest {
  scopes: string[];
  account?: MsalAccountInfo;
}

export interface MsalInstance {
  getAllAccounts(): MsalAccountInfo[];
  acquireTokenSilent(request: MsalTokenRequest): Promise<AuthenticationResult>;
  acquireTokenPopup(request: MsalTokenRequest): Promise<AuthenticationResult>;
  acquireTokenRedirect(request: MsalTokenRequest): Promise<void>;
}

/** Configuration for MsalTokenProvider. */
export interface MsalTokenProviderConfig {
  /**
   * An initialized MSAL PublicClientApplication instance (via `@azure/msal-browser`
   * or the `useMsal` hook from `@azure/msal-react`).
   */
  instance: MsalInstance;
  /** Scopes used when acquiring the application/api token. */
  apiScopes: string[];
  /**
   * Scopes used when acquiring a Microsoft Graph token.
   * Defaults to `['Sites.Read.All']`.
   */
  graphScopes?: string[];
  /**
   * Fallback behaviour when `acquireTokenSilent` fails. Accepts either a single
   * mode or an ordered list of modes to try in sequence before rejecting.
   * - `'popup'`    → try `acquireTokenPopup`.
   * - `'redirect'` → try `acquireTokenRedirect`.
   * - `'none'`    → rethrow the silent error without further interaction.
   *
   * When an array is provided, each mode is attempted in order until one
   * succeeds. If `'none'` appears in the list, it short-circuits and rethrows
   * the most recent error. Defaults to `['popup', 'redirect']`.
   */
  interactionMode?: InteractionMode | InteractionMode[];
  /**
   * Optional callback invoked when all acquisition strategies fail.
   * Useful for forcing logout or surfacing errors.
   */
  onAcquireFailed?: (error: unknown) => void | Promise<void>;
}

/** Interaction fallback strategy for token acquisition. */
export type InteractionMode = "popup" | "redirect" | "none";

/** Result of `refreshTokens` — both tokens that were acquired. */
export interface RefreshTokensResult {
  apiToken: AuthenticationResult;
  graphToken: AuthenticationResult;
}

/** Options for fetching the signed-in user's profile photo. */
export interface ProfilePhotoOptions {
  /**
   * Photo size in the form `"WIDTHxHEIGHT"`. Must match an available size.
   * Defaults to `"48x48"`.
   */
  size?: string;
  /** Override the base Graph URL. Defaults to `https://graph.microsoft.com/v1.0`. */
  graphBaseUrl?: string;
}

/** Result of fetching a profile photo. */
export interface ProfilePhotoResult {
  /** Raw Blob of the photo. */
  blob: Blob;
  /** Object URL (created via `URL.createObjectURL`). Caller is responsible for revoking. */
  objectUrl: string;
}
