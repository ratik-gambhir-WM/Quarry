import { describe, expect, test, beforeEach, mock } from "bun:test";
import { TokenManager } from "../auth";
import { InMemoryCache } from "../cache";
import { SharePointClientError } from "../errors";
import type { SharePointClientConfig } from "../../types";

const config: SharePointClientConfig = {
  tenantId: "test-tenant",
  clientId: "test-client",
  clientSecret: "test-secret",
};

describe("TokenManager", () => {
  let cache: InMemoryCache;
  let manager: TokenManager;

  beforeEach(() => {
    cache = new InMemoryCache();
    manager = new TokenManager(config, cache);
  });

  test("acquires token via client_credentials flow", async () => {
    const originalFetch = globalThis.fetch;
    globalThis.fetch = mock(() =>
      Promise.resolve(
        new Response(
          JSON.stringify({ access_token: "tok123", expires_in: 3600 }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      ),
    ) as unknown as typeof fetch;

    try {
      const token = await manager.getToken();
      expect(token).toBe("tok123");
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  test("returns cached token on second call", async () => {
    let callCount = 0;
    const originalFetch = globalThis.fetch;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({ access_token: "tok123", expires_in: 3600 }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    try {
      await manager.getToken();
      await manager.getToken();
      expect(callCount).toBe(1);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  test("throws SharePointClientError on HTTP failure", async () => {
    const originalFetch = globalThis.fetch;
    globalThis.fetch = mock(() =>
      Promise.resolve(new Response("Unauthorized", { status: 401 })),
    ) as unknown as typeof fetch;

    try {
      await expect(manager.getToken()).rejects.toThrow(SharePointClientError);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  test("throws when token response is missing access_token", async () => {
    const originalFetch = globalThis.fetch;
    globalThis.fetch = mock(() =>
      Promise.resolve(
        new Response(JSON.stringify({ error: "invalid_grant" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      ),
    ) as unknown as typeof fetch;

    try {
      await expect(manager.getToken()).rejects.toThrow("missing access_token");
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  test("scopes cache per credential set on a shared cache backend", async () => {
    // Two managers with different credentials sharing the same cache must
    // not read each other's tokens.
    const managerA = new TokenManager(
      { tenantId: "t1", clientId: "c1", clientSecret: "s1" },
      cache,
    );
    const managerB = new TokenManager(
      { tenantId: "t2", clientId: "c2", clientSecret: "s2" },
      cache,
    );

    const originalFetch = globalThis.fetch;
    let call = 0;
    globalThis.fetch = mock(() => {
      call++;
      return Promise.resolve(
        new Response(
          JSON.stringify({
            access_token: call === 1 ? "token-A" : "token-B",
            expires_in: 3600,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    try {
      expect(await managerA.getToken()).toBe("token-A");
      expect(await managerB.getToken()).toBe("token-B");
      // Each manager should still hit its own cache on a second call.
      expect(await managerA.getToken()).toBe("token-A");
      expect(await managerB.getToken()).toBe("token-B");
      expect(call).toBe(2);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  test("does not store the client secret in the cache key", async () => {
    const secret = "super-secret-value";
    const local = new InMemoryCache();
    const m = new TokenManager(
      { tenantId: "t", clientId: "c", clientSecret: secret },
      local,
    );

    const originalFetch = globalThis.fetch;
    globalThis.fetch = mock(() =>
      Promise.resolve(
        new Response(
          JSON.stringify({ access_token: "tok", expires_in: 3600 }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      ),
    ) as unknown as typeof fetch;

    try {
      await m.getToken();
      // Inspect the cache backing store via a getter — the secret must not
      // appear in any key.
      const store = (local as unknown as { store: Map<string, unknown> }).store;
      for (const key of store.keys()) {
        expect(key.includes(secret)).toBe(false);
      }
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
