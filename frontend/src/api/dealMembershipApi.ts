import type {
  DealMembershipApi,
  DealPageOptions,
} from "../contracts/dealMemberships";
import type { NotebookTransport } from "./notebookApi";
const id = encodeURIComponent;
const root = (d: string) => `/api/v1/deals/${id(d)}`;
function query(options: DealPageOptions = {}) {
  const p = new URLSearchParams();
  if (options.cursor !== undefined) p.set("cursor", options.cursor);
  if (options.limit !== undefined) p.set("limit", String(options.limit));
  return p.size ? `?${p}` : "";
}
/** Both distributions share membership paths and payloads; server policy owns authorization. */
export function createDealMembershipApi(
  request: NotebookTransport["request"],
): DealMembershipApi {
  return {
    listDealMembers: (d, o) => request("GET", `${root(d)}/members${query(o)}`),
    addDealMember: (d, body) => request("POST", `${root(d)}/members`, body),
    changeDealMemberRole: (d, u, body) =>
      request("PATCH", `${root(d)}/members/${id(String(u))}`, body),
    removeDealMember: (d, u, revision) =>
      request("DELETE", `${root(d)}/members/${id(String(u))}?expectedRevision=${revision}`),
    transferDealOwnership: (d, body) =>
      request("POST", `${root(d)}/ownership-transfer`, body),
    listDealTeams: (d, o) => request("GET", `${root(d)}/teams${query(o)}`),
    createDealTeam: (d, body) => request("POST", `${root(d)}/teams`, body),
    renameDealTeam: (d, t, body) =>
      request("PATCH", `${root(d)}/teams/${id(t)}`, body),
    deleteDealTeam: (d, t, revision) => request("DELETE", `${root(d)}/teams/${id(t)}?expectedRevision=${revision}`),
    addDealTeamMember: (d, t, userId) =>
      request("POST", `${root(d)}/teams/${id(t)}/members`, { userId }),
    removeDealTeamMember: (d, t, u) =>
      request("DELETE", `${root(d)}/teams/${id(t)}/members/${id(String(u))}`),
    listDealActivity: (d, o) =>
      request("GET", `${root(d)}/activity${query(o)}`),
  };
}
