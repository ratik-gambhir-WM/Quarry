import { useId, useRef, useState } from "react";
import { Pencil, UsersRound } from "lucide-react";
import { runtime } from "@quarry/runtime";
import type { DealTeam } from "../../contracts/dealMemberships";
import { Button } from "../ui/stealth/button";
import { Field, FieldLabel } from "../ui/stealth/field";
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from "../ui/stealth/collapsible";
import { Input } from "../ui/input";
import type { DealConfirmation, DealMutation } from "./DealMemberRow";

export function DealTeamCard({ team, canManage, pending, mutate, dealId, onConfirm }: {
  team: DealTeam;
  canManage: boolean;
  pending: boolean;
  mutate: DealMutation;
  dealId: string;
  onConfirm: (value: DealConfirmation, trigger: HTMLElement | null) => void;
}) {
  const fieldId = useId();
  const deleteTrigger = useRef<HTMLButtonElement>(null);
  const [name, setName] = useState(team.name);
  return (
    <Collapsible className="deal-team-card">
      <div className="deal-team-heading">
        <span className="deal-team-icon" aria-hidden="true"><UsersRound size={21} /></span>
        <div className="min-w-0 flex-1">
          <p className="deal-settings-eyebrow">Deal team</p>
          <h2 className="deal-team-name">{team.name}</h2>
          <p className="deal-team-count">{team.userIds.length} {team.userIds.length === 1 ? "member" : "members"} assigned</p>
        </div>
        {canManage ? <CollapsibleTrigger variant="inline" className="deal-team-manage" disabled={pending}><span className="inline-flex items-center gap-1.5"><Pencil size={13} aria-hidden="true" />Manage team</span></CollapsibleTrigger> : null}
      </div>
      {canManage ? <CollapsiblePanel contentClassName="deal-team-editor">
        <form onSubmit={(event) => {
          event.preventDefault();
          void mutate(() => runtime.api.renameDealTeam(dealId, team.teamId, { name: name.trim(), expectedRevision: team.revision }), "Team renamed.");
        }} className="flex flex-wrap items-end gap-2">
          <Field className="min-w-0 flex-1" disabled={pending}>
            <FieldLabel htmlFor={fieldId}>Team name</FieldLabel>
            <Input id={fieldId} required maxLength={100} value={name} onChange={(event) => setName(event.target.value)} />
          </Field>
          <Button type="submit" disabled={pending || !name.trim() || name.trim() === team.name}>Rename team</Button>
        </form>
        <div className="flex flex-wrap items-center justify-between gap-3">
          <p className="text-xs text-muted">Deleting the team keeps everyone’s deal access.</p>
          <Button ref={deleteTrigger} variant="ghost" className="text-destructive" disabled={pending} onClick={() => onConfirm({
            title: `Delete ${team.name}?`,
            description: "Team assignments will be removed. Everyone will keep their deal access and uploaded documents.",
            action: () => runtime.api.deleteDealTeam(dealId, team.teamId),
          }, deleteTrigger.current)}>Delete team</Button>
        </div>
      </CollapsiblePanel> : null}
    </Collapsible>
  );
}
