import { useRef } from "react";
import { ArrowRightLeft, MoreHorizontal, UserMinus, UsersRound } from "lucide-react";
import { runtime } from "@quarry/runtime";
import type { DealMember, DealTeam } from "../../contracts/dealMemberships";
import type { SavedDeal } from "../../data/dealExtraction";
import { Avatar, AvatarFallback } from "../ui/avatar";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "../ui/dropdown-menu";
import { DealMemberPermissions } from "./DealMemberPermissions";

export type DealMutation = (action: () => Promise<void>, success: string) => Promise<void>;
export type DealConfirmation = { title: string; description: string; action: () => Promise<void> };

export function DealMemberRow({ member, deal, teams, actorId, pending, mutate, onConfirm }: {
  member: DealMember;
  deal: SavedDeal;
  teams: DealTeam[];
  actorId: number | undefined;
  pending: boolean;
  mutate: DealMutation;
  onConfirm: (value: DealConfirmation, trigger: HTMLElement | null) => void;
}) {
  const trigger = useRef<HTMLButtonElement>(null);
  const openingConfirmation = useRef(false);
  const name = `${member.firstName} ${member.lastName}`.trim() || member.username;
  const assignedTeams = teams.filter((team) => team.userIds.includes(member.userId));
  const canTransfer = deal.capabilities?.transfer && member.role !== "owner";
  const canRemove = member.role !== "owner" && (deal.role === "owner" || member.userId === actorId);
  const canAssign = deal.capabilities?.manageTeams && teams.length > 0;
  function confirm(value: DealConfirmation) {
    openingConfirmation.current = true;
    onConfirm(value, trigger.current);
  }
  return (
    <li className="deal-member-row">
      <Avatar className="deal-member-avatar" aria-hidden="true">
        <AvatarFallback>{`${member.firstName.slice(0, 1)}${member.lastName.slice(0, 1)}`.toUpperCase() || member.username.slice(0, 2).toUpperCase()}</AvatarFallback>
      </Avatar>
      <div className="deal-member-identity">
        <p className="deal-member-name">{name}{member.userId === actorId ? <span className="deal-member-you">You</span> : null}</p>
        <p className="deal-member-username">@{member.username}</p>
        <p className="deal-member-assignment">{assignedTeams.length ? assignedTeams.map((team) => team.name).join(", ") : "Deal access only"}</p>
      </div>
      <div className="deal-member-controls">
        <DealMemberPermissions
          role={member.role}
          username={member.username}
          editable={Boolean(deal.capabilities?.manageMembers)}
          disabled={pending}
          onChange={(role) => void mutate(() => runtime.api.changeDealMemberRole(deal.dealId, member.userId, { role, expectedRevision: member.revision }), "Member role updated.")}
        />
        {canTransfer || canRemove || canAssign ? (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button ref={trigger} type="button" className="deal-member-more" disabled={pending} aria-label={`Actions for ${member.username}`}>
                <MoreHorizontal size={16} aria-hidden="true" />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-56" onCloseAutoFocus={(event) => {
              if (openingConfirmation.current) { event.preventDefault(); openingConfirmation.current = false; }
            }}>
              {canAssign ? teams.map((team) => {
                const assigned = team.userIds.includes(member.userId);
                return <DropdownMenuItem key={team.teamId} onSelect={() => void mutate(
                  () => assigned
                    ? runtime.api.removeDealTeamMember(deal.dealId, team.teamId, member.userId)
                    : runtime.api.addDealTeamMember(deal.dealId, team.teamId, member.userId),
                  assigned ? "Team assignment removed." : "Team member assigned.",
                )}><UsersRound aria-hidden="true" />{assigned ? `Remove from ${team.name}` : `Add to ${team.name}`}</DropdownMenuItem>;
              }) : null}
              {canAssign && (canTransfer || canRemove) ? <DropdownMenuSeparator /> : null}
              {canTransfer ? <DropdownMenuItem onSelect={() => confirm({
                title: `Transfer ownership to ${member.firstName}?`,
                description: "You will become a manager. Only the new owner can manage deal access or transfer ownership again.",
                action: () => runtime.api.transferDealOwnership(deal.dealId, { userId: member.userId, expectedAccessRevision: deal.accessRevision ?? 0 }),
              })}><ArrowRightLeft aria-hidden="true" />Transfer ownership</DropdownMenuItem> : null}
              {canRemove ? <DropdownMenuItem variant="destructive" onSelect={() => confirm({
                title: member.userId === actorId ? "Leave this deal?" : `Remove ${member.firstName}?`,
                description: "Access and team assignments will be removed. Uploaded documents and activity will remain. Downloaded copies cannot be retracted.",
                action: () => runtime.api.removeDealMember(deal.dealId, member.userId),
              })}><UserMinus aria-hidden="true" />{member.userId === actorId ? "Leave deal" : "Remove member"}</DropdownMenuItem> : null}
            </DropdownMenuContent>
          </DropdownMenu>
        ) : null}
      </div>
    </li>
  );
}
