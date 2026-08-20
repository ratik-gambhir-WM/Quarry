import { describe, expect, test } from "bun:test";
import {
  buildGraphRequest,
  buildGraphUrl,
  isUserScopedGraphPath,
} from "../graph";

const PROXY = { baseUrl: "https://westmonroe-cloud.com" };

describe("isUserScopedGraphPath", () => {
  test("matches /me paths", () => {
    expect(isUserScopedGraphPath("/me/joinedTeams")).toBe(true);
  });

  test("matches /users/{id} paths", () => {
    expect(isUserScopedGraphPath("/users/alex@example.com/joinedTeams")).toBe(
      true,
    );
  });

  test("does not match the /users collection endpoint", () => {
    expect(isUserScopedGraphPath("/users")).toBe(false);
  });

  test("does not match resource-scoped paths", () => {
    expect(isUserScopedGraphPath("/drives/drive-id/root")).toBe(false);
  });
});

describe("buildGraphUrl", () => {
  test("uses direct Microsoft Graph by default", () => {
    expect(buildGraphUrl("/me/joinedTeams")).toBe(
      "https://graph.microsoft.com/v1.0/me/joinedTeams",
    );
  });

  test("maps /me paths to the user graph proxy", () => {
    expect(buildGraphUrl("/me/joinedTeams", { userGraphProxy: PROXY })).toBe(
      "https://westmonroe-cloud.com/users/me/graph/joinedTeams",
    );
  });

  test("maps /me paths to a configured proxy user id", () => {
    expect(
      buildGraphUrl("/me/photos/48x48/$value", {
        userGraphProxy: {
          baseUrl: "https://westmonroe-cloud.com/",
          userId: "alex@example.com",
        },
      }),
    ).toBe(
      "https://westmonroe-cloud.com/users/alex%40example.com/graph/photos/48x48/$value",
    );
  });

  test("preserves /users/{id} paths through the proxy", () => {
    expect(
      buildGraphUrl("/users/user-123/joinedTeams?$top=5", {
        userGraphProxy: PROXY,
      }),
    ).toBe(
      "https://westmonroe-cloud.com/users/user-123/graph/joinedTeams?$top=5",
    );
  });

  test("does not proxy non-user Graph paths", () => {
    expect(buildGraphUrl("/drives/drive-id/root", { userGraphProxy: PROXY })).toBe(
      "https://graph.microsoft.com/v1.0/drives/drive-id/root",
    );
  });

  test("can proxy explicitly user-context operations that are not /me paths", () => {
    expect(
      buildGraphUrl(
        "/teams/team-id/channels?$filter=id eq 'channel-id'",
        { userGraphProxy: PROXY },
        { requiresUserContext: true },
      ),
    ).toBe(
      "https://westmonroe-cloud.com/users/me/graph/teams/team-id/channels?$filter=id eq 'channel-id'",
    );
  });
});

describe("buildGraphRequest", () => {
  test("adds Authorization for direct Graph calls", () => {
    const request = buildGraphRequest("/me/joinedTeams", {
      token: "Bearer token",
    });

    expect((request.init.headers as Record<string, string>).Authorization).toBe(
      "Bearer token",
    );
  });

  test("omits Authorization for proxy calls", () => {
    const request = buildGraphRequest(
      "/me/joinedTeams",
      { token: "Bearer token", userGraphProxy: PROXY },
      { headers: { "Content-Type": "application/json" } },
      { requiresUserContext: true },
    );

    const headers = request.init.headers as Record<string, string>;
    expect(headers.Authorization).toBeUndefined();
    expect(headers["Content-Type"]).toBe("application/json");
  });

  test("strips existing Authorization headers for proxy calls", () => {
    const request = buildGraphRequest(
      "/me/joinedTeams",
      { userGraphProxy: PROXY },
      { headers: { Authorization: "Bearer token" } },
      { requiresUserContext: true },
    );

    expect(
      (request.init.headers as Record<string, string>).Authorization,
    ).toBeUndefined();
  });

  test("throws for direct Graph calls without a token", () => {
    expect(() => buildGraphRequest("/me/joinedTeams", {})).toThrow(
      "Graph token is required",
    );
  });
});
