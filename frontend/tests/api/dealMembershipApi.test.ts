import { afterEach, describe, expect, it, vi } from "vitest";
import { createDealMembershipApi } from "@/api/dealMembershipApi";
import { httpQuarryApi } from "@/api/httpQuarryApi";
import { createTauriQuarryApi } from "@/api/tauriQuarryApi";
afterEach(() => vi.unstubAllGlobals());
describe("shared deal membership contracts", () => {
  it("encodes every nested resource, pagination and optimistic revision", async () => {
    const request = vi.fn().mockResolvedValue(undefined);
    const api = createDealMembershipApi(request);
    await api.listDealMembers("DEAL /one", { cursor: "a+b", limit: 30 });
    await api.addDealMember("DEAL /one", { email: "alice@westmonroe.com", role: "viewer" });
    await api.changeDealMemberRole("DEAL /one", 2, {
      role: "contributor",
      expectedRevision: 4,
    });
    await api.removeDealMember("DEAL /one", 2);
    await api.transferDealOwnership("DEAL /one", {
      userId: 2,
      expectedAccessRevision: 9,
    });
    await api.listDealTeams("DEAL /one");
    await api.createDealTeam("DEAL /one", { name: "Platform" });
    await api.renameDealTeam("DEAL /one", "team /one", {
      name: "Delivery",
      expectedRevision: 2,
    });
    await api.deleteDealTeam("DEAL /one", "team /one");
    await api.addDealTeamMember("DEAL /one", "team /one", 2);
    await api.removeDealTeamMember("DEAL /one", "team /one", 2);
    await api.listDealActivity("DEAL /one", {
      cursor: "time_event",
      limit: 100,
    });
    expect(request.mock.calls).toEqual([
      ["GET", "/api/v1/deals/DEAL%20%2Fone/members?cursor=a%2Bb&limit=30"],
      [
        "POST",
        "/api/v1/deals/DEAL%20%2Fone/members",
        { email: "alice@westmonroe.com", role: "viewer" },
      ],
      [
        "PATCH",
        "/api/v1/deals/DEAL%20%2Fone/members/2",
        { role: "contributor", expectedRevision: 4 },
      ],
      ["DELETE", "/api/v1/deals/DEAL%20%2Fone/members/2"],
      [
        "POST",
        "/api/v1/deals/DEAL%20%2Fone/ownership-transfer",
        { userId: 2, expectedAccessRevision: 9 },
      ],
      ["GET", "/api/v1/deals/DEAL%20%2Fone/teams"],
      ["POST", "/api/v1/deals/DEAL%20%2Fone/teams", { name: "Platform" }],
      [
        "PATCH",
        "/api/v1/deals/DEAL%20%2Fone/teams/team%20%2Fone",
        { name: "Delivery", expectedRevision: 2 },
      ],
      ["DELETE", "/api/v1/deals/DEAL%20%2Fone/teams/team%20%2Fone"],
      [
        "POST",
        "/api/v1/deals/DEAL%20%2Fone/teams/team%20%2Fone/members",
        { userId: 2 },
      ],
      ["DELETE", "/api/v1/deals/DEAL%20%2Fone/teams/team%20%2Fone/members/2"],
      [
        "GET",
        "/api/v1/deals/DEAL%20%2Fone/activity?cursor=time_event&limit=100",
      ],
    ]);
  });
  it("sends email-only member additions through both adapters", async () => {
    const input = { email: "member@westmonroe.com", role: "viewer" as const };
    const fetch = vi.fn().mockResolvedValue(new Response("null", { status: 201 }));
    vi.stubGlobal("fetch", fetch);
    await httpQuarryApi.addDealMember("DEAL-1", input);
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual(input);
    const transport = {
      get: vi.fn(), post: vi.fn().mockResolvedValue(null), delete: vi.fn(), getPdf: vi.fn(),
      postPowerPoint: vi.fn(), postMultipart: vi.fn(), subscribeJob: vi.fn(),
    };
    await createTauriQuarryApi(transport).addDealMember("DEAL-1", input);
    expect(transport.post).toHaveBeenCalledWith("/api/v1/deals/DEAL-1/members", input);
  });
  it("carries PATCH, empty DELETE and both search scopes through the HTTP adapter", async () => {
    const fetch = vi
      .fn()
      .mockImplementation(() =>
        Promise.resolve(new Response(null, { status: 204 })),
      );
    vi.stubGlobal("fetch", fetch);
    await httpQuarryApi.changeDealMemberRole("DEAL-1", 2, {
      role: "viewer",
      expectedRevision: 1,
    });
    await expect(
      httpQuarryApi.removeDealMember("DEAL-1", 2),
    ).resolves.toBeUndefined();
    await httpQuarryApi.searchDealDocumentChunksByKeyword("DEAL /one", {
      queryText: "scope",
      limit: 5,
    });
    await httpQuarryApi.searchDocumentChunksByKeyword({
      queryText: "scope",
      limit: 5,
    });
    await httpQuarryApi.searchDealDocumentChunksByVector("DEAL-1", {
      queryEmbedding: [1, 0],
      limit: 5,
    });
    expect(fetch.mock.calls.map(([url, init]) => [url, init.method])).toEqual([
      ["/api/v1/deals/DEAL-1/members/2", "PATCH"],
      ["/api/v1/deals/DEAL-1/members/2", "DELETE"],
      ["/api/v1/deals/DEAL%20%2Fone/documents/search/keyword", "POST"],
      ["/api/v1/documents/search/keyword", "POST"],
      ["/api/v1/deals/DEAL-1/documents/search/vector", "POST"],
    ]);
    expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual({
      role: "viewer",
      expectedRevision: 1,
    });
  });
  it("carries the same operations through the desktop adapter and fails explicitly without PATCH", async () => {
    const transport = {
      get: vi.fn(),
      post: vi.fn(),
      patch: vi.fn(),
      delete: vi.fn().mockResolvedValue(undefined),
      getPdf: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart: vi.fn(),
      subscribeJob: vi.fn(),
    };
    const api = createTauriQuarryApi(transport);
    await api.changeDealMemberRole("DEAL-1", 2, {
      role: "manager",
      expectedRevision: 7,
    });
    await api.removeDealMember("DEAL-1", 2);
    await api.searchDealDocumentChunksByKeyword("DEAL /one", {
      queryText: "scope",
      limit: 5,
    });
    await api.searchDocumentChunksByKeyword({ queryText: "scope", limit: 5 });
    expect(transport.patch).toHaveBeenCalledWith(
      "/api/v1/deals/DEAL-1/members/2",
      { role: "manager", expectedRevision: 7 },
    );
    expect(transport.delete).toHaveBeenCalledWith(
      "/api/v1/deals/DEAL-1/members/2",
    );
    expect(transport.post.mock.calls.map(([path]) => path)).toEqual([
      "/api/v1/deals/DEAL%20%2Fone/documents/search/keyword",
      "/api/v1/documents/search/keyword",
    ]);
    const unsupported = createTauriQuarryApi({
      ...transport,
      patch: undefined,
    });
    await expect(
      unsupported.renameDealTeam("DEAL-1", "team", {
        name: "New",
        expectedRevision: 1,
      }),
    ).rejects.toThrow("PATCH transport unavailable");
  });
});

it("maps hybrid text-in contracts, encoding and cancellation through HTTP and desktop", async () => {
  const response = { results: [{ chunkId: "c", dealId: "DEAL-1", fileId: "f", versionId: "v",
    text: "Synthetic passage", chunkIndex: 1, pageStart: 1, pageEnd: 1, sectionPath: "Renewals", matchSources: ["semantic"] }] };
  const fetch = vi.fn().mockImplementation(() => Promise.resolve(Response.json(response)));
  vi.stubGlobal("fetch", fetch);
  const input = { queryText: "Renewal risks?", limit: 30 };
  const controller = new AbortController();
  expect(await httpQuarryApi.searchDealDocumentChunksHybrid("DEAL /one", input, controller.signal)).toEqual(response);
  expect(await httpQuarryApi.searchDocumentChunksHybrid(input)).toEqual(response);
  expect(fetch.mock.calls.map(([path]) => path)).toEqual([
    "/api/v1/deals/DEAL%20%2Fone/documents/search/hybrid", "/api/v1/documents/search/hybrid",
  ]);
  expect(JSON.parse(fetch.mock.calls[0][1].body)).toEqual(input);
  expect(fetch.mock.calls[0][1].signal).toBeInstanceOf(AbortSignal);
  const transport = { get: vi.fn(), post: vi.fn().mockResolvedValue(response), delete: vi.fn(),
    getPdf: vi.fn(), postPowerPoint: vi.fn(), postMultipart: vi.fn(), subscribeJob: vi.fn() };
  const desktop = createTauriQuarryApi(transport);
  expect(await desktop.searchDealDocumentChunksHybrid("DEAL /one", input)).toEqual(response);
  expect(await desktop.searchDocumentChunksHybrid(input)).toEqual(response);
  expect(transport.post.mock.calls).toEqual([
    ["/api/v1/deals/DEAL%20%2Fone/documents/search/hybrid", input], ["/api/v1/documents/search/hybrid", input],
  ]);
});
