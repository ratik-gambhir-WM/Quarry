import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import { SharePointClient } from "../client";

describe("SharePointClient user graph proxy", () => {
  let originalFetch: typeof fetch;

  beforeEach(() => {
    originalFetch = globalThis.fetch;
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("uses proxy for joined teams without client credentials", async () => {
    let requestedUrl = "";
    globalThis.fetch = mock((url: string) => {
      requestedUrl = url;
      return Promise.resolve(
        new Response(
          JSON.stringify({ value: [{ id: "t1", displayName: "Team One" }] }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    const client = new SharePointClient({
      userGraphProxy: { baseUrl: "https://westmonroe-cloud.com" },
    });

    const teams = await client.getJoinedTeams();

    expect(teams[0].id).toBe("t1");
    expect(requestedUrl).toBe(
      "https://westmonroe-cloud.com/users/me/graph/joinedTeams",
    );
  });

  test("still requires credentials for direct Graph token acquisition", async () => {
    const client = new SharePointClient({});

    await expect(client.getDriveId({ teamsId: "t1" })).rejects.toThrow(
      "tenantId, clientId, and clientSecret are required",
    );
  });
});
