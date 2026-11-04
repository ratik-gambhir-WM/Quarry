import { useCallback, useId, useReducer, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { LockKeyhole, UserPlus, UsersRound } from "lucide-react";
import { runtime } from "@quarry/runtime";
import { sessionStore } from "../../auth/sessionStore";
import { useWorkspace } from "../../app/WorkspaceProvider";
import type { AssignableDealRole } from "../../contracts/dealMemberships";
import type { SavedDeal } from "../../data/dealExtraction";
import { useDealCollaboration } from "../../hooks/useDealCollaboration";
import { Button } from "../ui/stealth/button";
import { Field, FieldLabel } from "../ui/stealth/field";
import { SelectField } from "../ui/stealth/select-field";
import { EmptyState } from "../ui/stealth/empty-state";
import { Input } from "../ui/input";
import { AlertDialog, AlertDialogContent, AlertDialogDescription, AlertDialogTitle } from "../ui/stealth/alert-dialog";
import { DealMemberRow, type DealConfirmation } from "./DealMemberRow";
import { DealTeamCard } from "./DealTeamCard";
import "./deal-settings.css";

const roles: AssignableDealRole[] = ["manager", "contributor", "viewer"];
function RoleSelect({
  value,
  onChange,
  label,
  disabled,
}: {
  value: AssignableDealRole;
  onChange: (r: AssignableDealRole) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <SelectField<AssignableDealRole>
      label={label}
      value={value}
      disabled={disabled}
      onValueChange={onChange}
      className="w-full"
      options={roles.map((role) => ({
        value: role,
        label: role[0].toUpperCase() + role.slice(1),
      }))}
    />
  );
}
export function DealMembersPanel({ deal }: { deal: SavedDeal }) {
  const formId = useId();
  const cancelButton = useRef<HTMLButtonElement>(null);
  const { roster, reload } = useDealCollaboration(deal.dealId);
  const { loadDeal, forgetDeal } = useWorkspace();
  const navigate = useNavigate();
  const [workflow, dispatch] = useReducer(mutationReducer, { status: "idle" });
  const inFlight = useRef(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const confirmationTrigger = useRef<HTMLElement | null>(null);
  const pending = workflow.status === "pending";
  const error = workflow.status === "error" ? workflow.message : "";
  const message = workflow.status === "success" ? workflow.message : "";
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<AssignableDealRole>("contributor");
  const [teamName, setTeamName] = useState("");
  const [confirm, setConfirm] = useState<DealConfirmation | null>(null);
  const actorId = sessionStore.getSnapshot().session?.user.id;
  const capabilities = deal.capabilities;
  const mutate = useCallback(
    async (action: () => Promise<void>, success: string) => {
      if (inFlight.current) return;
      inFlight.current = true;
      dispatch({ type: "begin" });
      try {
        await action();
        setConfirm(null);
        dispatch({ type: "success", message: success });
        await loadDeal(deal.dealId);
        reload();
      } catch (e) {
        const status =
          typeof e === "object" && e !== null && "status" in e
            ? e.status
            : undefined;
        if (status === 404) {
          forgetDeal(deal.dealId);
          navigate("/hub/deals", { replace: true });
        }
        dispatch({
          type: "error",
          message:
            status === 409
              ? "This deal changed. The roster has been refreshed; review it and try again."
              : e instanceof Error
                ? e.message
                : "The change could not be saved. Try again.",
        });
        if (status === 409) reload();
      } finally {
        inFlight.current = false;
        heading.current?.focus();
      }
    },
    [loadDeal, deal.dealId, forgetDeal, navigate, reload],
  );
  function askForConfirmation(value: DealConfirmation, trigger: HTMLElement | null) {
    confirmationTrigger.current = trigger;
    setConfirm(value);
  }
  return (
    <section aria-labelledby="deal-members-heading" className="deal-settings-workspace">
      <div className="deal-settings-main">
        {roster.status === "ready" ? (
          roster.teams.length ? roster.teams.map((team) => (
            <DealTeamCard key={`${team.teamId}:${team.revision}`} team={team}
              canManage={Boolean(capabilities?.manageTeams)} pending={pending}
              mutate={mutate} dealId={deal.dealId} onConfirm={askForConfirmation} />
          )) : (
            <div className="deal-team-card deal-team-empty">
              <span className="deal-team-icon" aria-hidden="true"><UsersRound size={21} /></span>
              <div className="min-w-0 flex-1">
                <p className="deal-settings-eyebrow">Deal team</p>
                <h2 className="deal-team-name">Set up your team</h2>
                <p className="deal-team-count">Give the people working on this deal a shared team name.</p>
                {capabilities?.manageTeams ? (
                  <form className="mt-4 flex flex-wrap items-end gap-2" onSubmit={(event) => {
                    event.preventDefault();
                    void mutate(() => runtime.api.createDealTeam(deal.dealId, { name: teamName.trim() }), "Team created.");
                  }}>
                    <Field className="min-w-0 flex-1" disabled={pending}>
                      <FieldLabel htmlFor={`${formId}-team`}>Team name</FieldLabel>
                      <Input id={`${formId}-team`} required maxLength={100} value={teamName} onChange={(event) => setTeamName(event.target.value)} />
                    </Field>
                    <Button type="submit" variant="primary" loading={pending} disabled={!teamName.trim()}>Create team</Button>
                  </form>
                ) : <p className="mt-3 text-xs text-muted">A manager can create the deal team.</p>}
              </div>
            </div>
          )
        ) : roster.status === "loading" ? <div className="deal-team-loading" aria-hidden="true"><span /><div><span /><span /></div></div> : null}
        <div className="deal-roster-card">
          <header className="deal-roster-header">
            <div>
              <h2 ref={heading} tabIndex={-1} id="deal-members-heading">Deal members</h2>
              <p>Everyone with access to this deal.</p>
            </div>
            {roster.status === "ready" ? <span className="deal-roster-count" aria-label={`${roster.members.length} deal members`}>{roster.members.length}</span> : null}
          </header>
          <div className="deal-settings-feedback">
            <p role="status" aria-live="polite">{pending ? "Saving changes…" : message}</p>
            {error && !confirm ? <p role="alert" className="text-destructive">{error}</p> : null}
          </div>
          {roster.status === "loading" ? <div className="deal-roster-loading" role="status">Loading members and teams…</div>
            : roster.status === "error" ? <div className="deal-roster-loading" role="alert"><p>{roster.message}</p><Button className="mt-3" onClick={reload}>Retry roster</Button></div>
            : roster.members.length ? (
              <ul className="deal-members-list" aria-label="Deal members">
                {roster.members.map((member) => <DealMemberRow key={member.userId} member={member} deal={deal} teams={roster.teams}
                  actorId={actorId} pending={pending} mutate={mutate} onConfirm={askForConfirmation} />)}
              </ul>
            ) : <EmptyState heading="No members to display" description="Refresh the roster to check deal access." icon={null} size="sm" />}
          <footer className="deal-roster-footer"><LockKeyhole size={13} aria-hidden="true" />Team assignments organize people; deal permissions control access.</footer>
        </div>
      </div>
      <aside className="deal-settings-aside" aria-label="Member management">
        {capabilities?.manageMembers && roster.status === "ready" ? (
          <section className="deal-invite-card" aria-labelledby={`${formId}-invite-title`}>
            <span className="deal-aside-icon" aria-hidden="true"><UserPlus size={18} /></span>
            <h2 id={`${formId}-invite-title`}>Add a member</h2>
            <p>Add a registered Quarry user to this deal.</p>
            <form className="mt-5 space-y-4" onSubmit={(event) => {
              event.preventDefault();
              void mutate(() => runtime.api.addDealMember(deal.dealId, { email: email.trim(), role }), "Member added.");
            }}>
              <Field disabled={pending}>
                <FieldLabel htmlFor={`${formId}-email`}>Email address</FieldLabel>
                <Input id={`${formId}-email`} type="email" required maxLength={320} value={email} onChange={(event) => setEmail(event.target.value)} autoComplete="off" />
              </Field>
              <RoleSelect label="New member role" value={role} onChange={setRole} disabled={pending} />
              <Button className="w-full" variant="primary" type="submit" loading={pending} disabled={!email.trim()} leadingIcon={<UserPlus size={14} />}>Add member</Button>
            </form>
          </section>
        ) : null}
        <section className="deal-access-guide" aria-labelledby={`${formId}-permissions-title`}>
          <h2 id={`${formId}-permissions-title`}>About permissions</h2>
          <dl>
            <div><dt>Manager</dt><dd>Edits deal details and organizes the team.</dd></div>
            <div><dt>Contributor</dt><dd>Uploads and works with deal documents.</dd></div>
            <div><dt>Viewer</dt><dd>Reads shared deal documents.</dd></div>
          </dl>
          <p>Only the owner can change deal permissions. Notebooks and chats stay personal.</p>
        </section>
      </aside>
      <AlertDialog open={confirm !== null} onOpenChange={(open, details) => {
        if (!open && pending) details.cancel();
        else if (!open) setConfirm(null);
      }}>
        <AlertDialogContent initialFocus={cancelButton} finalFocus={() => confirmationTrigger.current?.isConnected ? confirmationTrigger.current : heading.current}>
          <AlertDialogTitle>{confirm?.title}</AlertDialogTitle>
          <AlertDialogDescription>{confirm?.description}</AlertDialogDescription>
          {error ? <p role="alert" className="mt-3 text-sm text-destructive">{error}</p> : null}
          <div className="mt-5 flex justify-end gap-2">
            <Button ref={cancelButton} variant="secondary" disabled={pending} onClick={() => setConfirm(null)}>Cancel</Button>
            <Button variant="primary" loading={pending} loadingText="Saving…" onClick={() => { if (confirm) void mutate(confirm.action, "Deal access updated."); }}>Confirm change</Button>
          </div>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}

type MutationState =
  | { status: "idle" | "pending" }
  | { status: "success" | "error"; message: string };
type MutationAction =
  | { type: "begin" }
  | { type: "success" | "error"; message: string };
function mutationReducer(
  _state: MutationState,
  action: MutationAction,
): MutationState {
  return action.type === "begin"
    ? { status: "pending" }
    : { status: action.type, message: action.message };
}
