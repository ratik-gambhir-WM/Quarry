export type DealRole = "owner" | "manager" | "contributor" | "viewer";
export type AssignableDealRole = Exclude<DealRole, "owner">;
export type DealCapabilities = {
  read: boolean;
  upload: boolean;
  edit: boolean;
  manageTeams: boolean;
  manageMembers: boolean;
  transfer: boolean;
  archive: boolean;
  localSource: boolean;
};
export type DealAccess = {
  role: DealRole;
  accessRevision: number;
  createdByUserId: number;
  archived: boolean;
  capabilities: DealCapabilities;
};
export type DealMember = {
  userId: number;
  username: string;
  firstName: string;
  lastName: string;
  role: DealRole;
  revision: number;
  joinedAt: string;
  teamIds: string[];
};
export type DealTeam = {
  teamId: string;
  name: string;
  revision: number;
  userIds: number[];
};
export type DealActivityEvent = {
  eventId: string;
  actorUserId: number | null;
  actorName: string | null;
  kind: string;
  targetId: string | null;
  createdAt: string;
};
export type DealPage<T> = { items: T[]; nextCursor: string | null };
export type DealPageOptions = { cursor?: string; limit?: number };
export interface DealMembershipApi {
  listDealMembers(
    dealId: string,
    options?: DealPageOptions,
  ): Promise<DealPage<DealMember>>;
  addDealMember(
    dealId: string,
    input: { email: string; role: AssignableDealRole },
  ): Promise<void>;
  changeDealMemberRole(
    dealId: string,
    userId: number,
    input: { role: AssignableDealRole; expectedRevision: number },
  ): Promise<void>;
  removeDealMember(dealId: string, userId: number, expectedRevision: number): Promise<void>;
  transferDealOwnership(
    dealId: string,
    input: { userId: number; expectedAccessRevision: number },
  ): Promise<void>;
  listDealTeams(
    dealId: string,
    options?: DealPageOptions,
  ): Promise<DealPage<DealTeam>>;
  createDealTeam(dealId: string, input: { name: string }): Promise<void>;
  renameDealTeam(
    dealId: string,
    teamId: string,
    input: { name: string; expectedRevision: number },
  ): Promise<void>;
  deleteDealTeam(dealId: string, teamId: string, expectedRevision: number): Promise<void>;
  addDealTeamMember(
    dealId: string,
    teamId: string,
    userId: number,
  ): Promise<void>;
  removeDealTeamMember(
    dealId: string,
    teamId: string,
    userId: number,
  ): Promise<void>;
  listDealActivity(
    dealId: string,
    options?: DealPageOptions,
  ): Promise<DealPage<DealActivityEvent>>;
}
