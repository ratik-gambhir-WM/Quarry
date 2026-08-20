# sharepoint-library

A generic TypeScript library for interacting with Microsoft Graph API / SharePoint. Designed to be reused across multiple projects.

The package ships two subpath entry points:

| Import                       | Runtime    | Purpose                                                                  |
| ---------------------------- | ---------- | ------------------------------------------------------------------------ |
| `sharepoint-library/server`  | Node / Bun | `SharePointClient` + `client_credentials` token manager (app-only auth). |
| `sharepoint-library/browser` | Browser    | `MsalTokenProvider` + Graph helpers for user-delegated (MSAL) auth.      |

The browser entry has an **optional** peer dependency on `@azure/msal-browser` — it's only required if you use `MsalTokenProvider`.

## Features

### `sharepoint-library/server`

- **`SharePointClient` class** — single entry point for all server-side operations
- **Auto-managed OAuth tokens** — client_credentials flow with automatic caching and refresh
- **Per-call token override** — pass a user-delegated token (e.g. from the browser helper) to any method
- **Optional user Graph proxy** — route user-context Graph calls through `/users/{id}/graph/...` without sending a Graph token
- **Pluggable cache** — `CacheAdapter` interface with a built-in `InMemoryCache` default; swap in Redis or any backend
- **Teams membership** — check team/channel membership, batch-fetch channels
- **Drive operations** — resolve drive IDs, list folder children (async generator), check folder existence
- **File operations** — recursive file listing, generic file diffing, file download
- **SharePoint Search** — search files, sites, and folders via the Graph Search API
- **Configurable filters** — exclude folders, files, or file extensions during listing

### `sharepoint-library/browser`

- **`MsalTokenProvider` class** — wraps an MSAL `PublicClientApplication` to acquire API and Graph tokens with the silent → popup → redirect fallback chain
- **Fully configurable scopes** — caller passes `apiScopes` and `graphScopes`
- **No storage side-effects** — callers decide how to persist the returned tokens
- **`fetchTeamsProfilePhoto` helper** — fetches `/me/photos/{size}/$value` via Graph and returns a `Blob` + object URL

## Prerequisites

- [Bun](https://bun.sh/) >= 1.0 (for development and running `.ts` directly)
- An Azure AD application with:
  - `client_id`, `client_secret`, `tenant_id`
  - Microsoft Graph API permissions (e.g., `Sites.Read.All`, `Files.Read.All`)

## Setup

```bash
cd sharepoint-library
bun install
```

## Build

```bash
bun run build       # Compiles to dist/ via tsc
bun run type-check  # Type-check without emitting
```

## Running Tests

Tests use Bun's built-in test runner:

```bash
bun test
```

Test files are organized under `tests/` mirroring the `src/` folder structure:

```
tests/
  core/
    auth.test.ts       # TokenManager: acquire, cache, error handling
    cache.test.ts      # InMemoryCache: get/set/delete/clear, TTL expiration
    errors.test.ts     # SharePointClientError: construction, instanceof
    utils.test.ts      # URL parsing, path normalization, exclusion checks
  services/
    files.test.ts      # diffFiles: add/remove/modify/edge cases
```

## Usage

### Basic Setup (server)

```typescript
import { SharePointClient } from "sharepoint-library/server";

const client = new SharePointClient({
  tenantId: process.env.AZUREAD_TENANT_ID,
  clientId: process.env.AZUREAD_CLIENT_ID,
  clientSecret: process.env.AZUREAD_CLIENT_SECRET,
});
```

### User Graph Proxy Setup (server)

For projects that proxy user-scoped Graph requests through the application host:

```typescript
const client = new SharePointClient({
  userGraphProxy: {
    baseUrl: "https://westmonroe-cloud.com",
  },
});

const teams = await client.getJoinedTeams();
// GET https://westmonroe-cloud.com/users/me/graph/joinedTeams
// No Authorization header is sent by this library.
```

Only user-context operations use the proxy, such as joined-team and membership checks. Resource operations like drives, files, downloads, and SharePoint search continue to call `https://graph.microsoft.com/v1.0/...` and still require a token or configured client credentials.

If proxy responses are safe to cache for a stable user/session namespace, pass `cacheKey`. When omitted, `/users/me` proxy responses are not cached to avoid sharing one user's data with another user through a shared cache.

### Basic Setup (browser — user-delegated auth)

```typescript
import { useMsal } from "@azure/msal-react";
import {
  MsalTokenProvider,
  fetchTeamsProfilePhoto,
} from "sharepoint-library/browser";

function useTokens() {
  const { instance } = useMsal();
  const provider = new MsalTokenProvider({
    instance,
    apiScopes: [`api://${import.meta.env.VITE_AZUREAD_CLIENT_ID}/dev/app.dev`],
    graphScopes: ["Sites.Read.All"],
  });

  async function refresh() {
    const { apiToken, graphToken } = await provider.refreshTokens();
    localStorage.setItem("token", apiToken.accessToken);
    localStorage.setItem("graphToken", graphToken.accessToken);
    return { apiToken, graphToken };
  }

  return { provider, refresh };
}

// Then later:
const { objectUrl } = await fetchTeamsProfilePhoto(
  localStorage.getItem("graphToken")!,
);
```

### Acquire a Token

```typescript
const token = await client.acquireToken();
```

### List Files in a SharePoint Folder

```typescript
const driveId = await client.getDriveId({
  teamsId: "team-id",
  channelId: "channel-id",
});
const files = await client.listFiles(driveId, "/Documents/Reports", {
  excludedExtensions: [".mp4", ".zip"],
  excludedFolders: ["archive"],
});
```

### Download a File

```typescript
const { buffer, size } = await client.downloadFile(driveId, "item-id");
```

### Search SharePoint

```typescript
const results = await client.searchFiles("quarterly report");
```

### Check Folder Existence

```typescript
const exists = await client.checkSharePointFolderExists(
  { teamsId: "team-id" },
  "https://company.sharepoint.com/sites/team/Shared%20Documents/Reports",
);
```

### File Sync (Diff)

```typescript
const { diff, driveId } = await client.getFilesForSync(
  { teamsId: "team-id", channelId: "channel-id" },
  "https://company.sharepoint.com/sites/team/Shared%20Documents/Data",
  existingDbFiles,
  {
    getNewId: (driveItem) => driveItem.id,
    getExistingId: (dbFile) => dbFile.sharepointId,
    shouldUpdate: (driveItem, dbFile) =>
      driveItem.lastModifiedDateTime !== dbFile.updatedAt,
  },
);
// diff.added, diff.modified, diff.removed
```

### Using a Custom Cache

```typescript
import { SharePointClient, type CacheAdapter } from "sharepoint-library/server";

class RedisCache implements CacheAdapter {
  async get<T>(key: string) {
    /* ... */
  }
  async set<T>(key: string, value: T, ttlMs?: number) {
    /* ... */
  }
  async delete(key: string) {
    /* ... */
  }
  async clear() {
    /* ... */
  }
}

const client = new SharePointClient({
  tenantId: "...",
  clientId: "...",
  clientSecret: "...",
  cache: new RedisCache(),
  defaultCacheTtlMs: 600_000, // 10 minutes
});
```

### Per-Call Token Override

Every Graph-calling method accepts an optional `graphToken` parameter as its last argument. If provided, it overrides the auto-managed token:

```typescript
// Use a user-delegated token from the frontend
const teams = await client.getJoinedTeams(userGraphToken);
```

## Adding as a Dependency

In a Bun workspace, add to `package.json`:

```json
{
  "dependencies": {
    "sharepoint-library": "file:../../sharepoint-library"
  }
}
```

Then `bun install`.

## Project Structure

```
sharepoint-library/
  src/
    server/
      index.ts              # Server barrel exports
      client.ts             # SharePointClient class
      types/
        index.d.ts          # Server type declarations
      core/
        auth.ts             # TokenManager (client_credentials)
        cache.ts            # InMemoryCache
        errors.ts           # SharePointClientError
        utils.ts            # HTTP helpers, path utilities
      services/
        teams.ts            # Teams membership operations
        drives.ts           # Drive ID resolution, folder checks
        files.ts            # File listing, diffing, downloading
        search.ts           # SharePoint Search API
    browser/
      index.ts              # Browser barrel exports
      msalTokenProvider.ts  # User-delegated token acquisition via MSAL
      graph.ts              # Browser-side Graph helpers (profile photo)
      types.ts              # Browser type declarations
```

## Code Standards

- **TypeScript strict mode** — all code is strict-mode compliant
- **ES2025 target** — modern JavaScript features
- **ESM** — `"type": "module"` in package.json
- **No runtime dependencies** — uses native `fetch()` for all HTTP
- **Declaration file for types** — `src/types/index.d.ts` for clean separation
- **No `.js` extensions in imports** — imports use `.ts`-compatible paths (resolved by Bun)
- **No non-null assertions** (`!`) — guard clauses and explicit checks instead
- **Named types over inline** — all response shapes and options are declared in the types file
