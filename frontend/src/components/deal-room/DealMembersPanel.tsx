import { useCallback, useId, useReducer, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import { sessionStore } from "../../auth/sessionStore";
import { useWorkspace } from "../../app/WorkspaceProvider";
import type {
  AssignableDealRole,
  DealMember,
  DealTeam,
} from "../../contracts/dealMemberships";
import type { SavedDeal } from "../../data/dealExtraction";
import { useDealCollaboration } from "../../hooks/useDealCollaboration";
import { Button } from "../ui/stealth/button";
import { Field, FieldLabel } from "../ui/stealth/field";
import { SelectField } from "../ui/stealth/select-field";
import { EmptyState } from "../ui/stealth/empty-state";
import {
  Collapsible,
  CollapsibleTrigger,
  CollapsiblePanel,
} from "../ui/stealth/collapsible";
import { Input } from "../ui/input";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogTitle,
} from "../ui/stealth/alert-dialog";
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
      className="w-full sm:w-44"
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
  const { cacheDealResult, forgetDeal } = useWorkspace();
  const navigate = useNavigate();
  const [workflow, dispatch] = useReducer(mutationReducer, { status: "idle" });
  const inFlight = useRef(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const confirmationTrigger = useRef<HTMLElement | null>(null);
  const pending = workflow.status === "pending";
  const error = workflow.status === "error" ? workflow.message : "";
  const message = workflow.status === "success" ? workflow.message : "";
  const [username, setUsername] = useState("");
  const [role, setRole] = useState<AssignableDealRole>("contributor");
  const [teamName, setTeamName] = useState("");
  const [confirm, setConfirm] = useState<{
    title: string;
    description: string;
    action: () => Promise<void>;
  } | null>(null);
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
        const current = await runtime.api.getDeal(deal.dealId);
        cacheDealResult(current);
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
    [cacheDealResult, deal.dealId, forgetDeal, navigate, reload],
  );
  function askForConfirmation(value: NonNullable<typeof confirm>) {
    confirmationTrigger.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    setConfirm(value);
  }
  return (
    <section aria-labelledby="deal-members-heading" className="space-y-6">
      <div>
        <h2
          ref={heading}
          tabIndex={-1}
          id="deal-members-heading"
          className="type-h3"
        >
          Team and members
        </h2>
        <p className="mt-2 text-sm text-muted">
          Members share uploaded documents. Notebooks and chats remain personal.
          Teams organize members without granting access.
        </p>
      </div>
      <p role="status" aria-live="polite" className="text-sm">
        {message}
      </p>
      {error && !confirm ? (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      ) : null}
      {roster.status === "loading" ? (
        <p role="status">Loading members and teams…</p>
      ) : roster.status === "error" ? (
        <div role="alert">
          <p>{roster.message}</p>
          <Button onClick={reload}>Retry roster</Button>
        </div>
      ) : (
        <>
          <h3 className="font-semibold">Deal team</h3>
          {roster.teams.length === 0 ? (
            <EmptyState
              size="sm"
              variant="first-use"
              heading="No team yet."
              description={
                capabilities?.manageTeams
                  ? "Create a team to organize deal members."
                  : "A manager can create the deal team."
              }
              icon={null}
            />
          ) : null}
          <ul className="space-y-3" aria-label="Deal team">
            {roster.teams.map((team) => (
              <TeamRow
                key={`${team.teamId}:${team.revision}`}
                team={team}
                members={roster.members}
                canManage={Boolean(capabilities?.manageTeams)}
                pending={pending}
                mutate={mutate}
                dealId={deal.dealId}
              />
            ))}
          </ul>
          {capabilities?.manageTeams && roster.teams.length === 0 ? (
            <form
              className="flex max-w-3xl flex-col gap-3 sm:flex-row sm:items-end"
              onSubmit={(e) => {
                e.preventDefault();
                void mutate(
                  () =>
                    runtime.api.createDealTeam(deal.dealId, { name: teamName }),
                  "Team created.",
                );
              }}
            >
              <Field className="min-w-0 flex-1" disabled={pending}>
                <FieldLabel htmlFor={`${formId}-team`}>Team name</FieldLabel>
                <Input
                  id={`${formId}-team`}
                  required
                  maxLength={100}
                  value={teamName}
                  onChange={(e) => setTeamName(e.target.value)}
                />
              </Field>
              <Button
                variant="primary"
                type="submit"
                loading={pending}
                disabled={!teamName.trim()}
              >
                Create team
              </Button>
            </form>
          ) : null}
          <h3 className="font-semibold">Deal members</h3>
          <ul className="divide-y divide-border" aria-label="Deal members">
            {roster.members.map((member) => (
              <li
                key={member.userId}
                className="flex flex-wrap items-center justify-between gap-3 py-4"
              >
                <div>
                  <p className="font-medium">
                    {member.firstName} {member.lastName}
                  </p>
                  <p className="text-sm text-muted">
                    @{member.username} · {member.role}
                  </p>
                </div>
                <div className="flex flex-wrap items-end gap-2">
                  {capabilities?.manageMembers && member.role !== "owner" ? (
                    <RoleSelect
                      label={`Role for ${member.username}`}
                      value={member.role}
                      disabled={pending}
                      onChange={(role) =>
                        void mutate(
                          () =>
                            runtime.api.changeDealMemberRole(
                              deal.dealId,
                              member.userId,
                              { role, expectedRevision: member.revision },
                            ),
                          "Member role updated.",
                        )
                      }
                    />
                  ) : null}
                  {capabilities?.transfer && member.role !== "owner" ? (
                    <Button
                      variant="secondary"
                      disabled={pending}
                      onClick={() =>
                        askForConfirmation({
                          title: `Transfer ownership to ${member.firstName}?`,
                          description:
                            "You will become a manager. Only the new owner can manage deal access or transfer ownership again.",
                          action: () =>
                            runtime.api.transferDealOwnership(deal.dealId, {
                              userId: member.userId,
                              expectedAccessRevision: deal.accessRevision ?? 0,
                            }),
                        })
                      }
                    >
                      Transfer ownership
                    </Button>
                  ) : null}
                  {member.role !== "owner" &&
                  (deal.role === "owner" || member.userId === actorId) ? (
                    <Button
                      variant="secondary"
                      disabled={pending}
                      onClick={() =>
                        askForConfirmation({
                          title:
                            member.userId === actorId
                              ? "Leave this deal?"
                              : `Remove ${member.firstName}?`,
                          description:
                            "Access and team assignments will be removed. Uploaded documents and activity will remain. Downloaded copies cannot be retracted.",
                          action: () =>
                            runtime.api.removeDealMember(
                              deal.dealId,
                              member.userId,
                            ),
                        })
                      }
                    >
                      {member.userId === actorId
                        ? "Leave deal"
                        : "Remove member"}
                    </Button>
                  ) : null}
                </div>
              </li>
            ))}
          </ul>
          {capabilities?.manageMembers ? (
            <form
              className="flex max-w-3xl flex-col gap-3 sm:flex-row sm:items-end"
              onSubmit={(e) => {
                e.preventDefault();
                void mutate(
                  () =>
                    runtime.api.addDealMember(deal.dealId, { username, role }),
                  "Member added.",
                );
              }}
            >
              <Field className="min-w-0 flex-1" disabled={pending}>
                <FieldLabel htmlFor={`${formId}-username`}>
                  Registered username
                </FieldLabel>
                <Input
                  id={`${formId}-username`}
                  required
                  maxLength={64}
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  autoComplete="off"
                />
              </Field>
              <RoleSelect
                label="New member role"
                value={role}
                onChange={setRole}
                disabled={pending}
              />
              <Button
                variant="primary"
                type="submit"
                loading={pending}
                disabled={!username.trim()}
              >
                Add member
              </Button>
            </form>
          ) : null}

        </>
      )}
      <AlertDialog
        open={confirm !== null}
        onOpenChange={(open, details) => {
          if (!open && pending) details.cancel();
          else if (!open) setConfirm(null);
        }}
      >
        <AlertDialogContent
          initialFocus={cancelButton}
          finalFocus={() =>
            confirmationTrigger.current?.isConnected
              ? confirmationTrigger.current
              : heading.current
          }
        >
          <AlertDialogTitle>{confirm?.title}</AlertDialogTitle>
          <AlertDialogDescription>
            {confirm?.description}
          </AlertDialogDescription>
          {error ? (
            <p role="alert" className="mt-3 text-sm text-destructive">
              {error}
            </p>
          ) : null}
          <div className="mt-5 flex justify-end gap-2">
            <Button
              ref={cancelButton}
              variant="secondary"
              disabled={pending}
              onClick={() => setConfirm(null)}
            >
              Cancel
            </Button>
            <Button
              variant="primary"
              loading={pending}
              loadingText="Saving…"
              onClick={() => {
                if (confirm)
                  void mutate(confirm.action, "Deal access updated.");
              }}
            >
              Confirm change
            </Button>
          </div>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
function TeamRow({
  team,
  members,
  canManage,
  pending,
  mutate,
  dealId,
}: {
  team: DealTeam;
  members: DealMember[];
  canManage: boolean;
  pending: boolean;
  mutate: (action: () => Promise<void>, success: string) => Promise<void>;
  dealId: string;
}) {
  const fieldId = useId();
  const [name, setName] = useState(team.name);
  const [selected, setSelected] = useState("");
  return (
    <li className="rounded-lg border border-border p-4">
      <h4 className="mb-2 font-semibold break-words">{team.name}</h4>
      <Collapsible defaultOpen>
        <CollapsibleTrigger>
          {team.name} · {team.userIds.length} members
        </CollapsibleTrigger>
        <CollapsiblePanel contentClassName="pt-4 space-y-3">
          <ul aria-label={`${team.name} members`}>
            {team.userIds.map((id) => {
              const member = members.find((m) => m.userId === id);
              return (
                <li
                  className="flex items-center justify-between gap-2 py-1"
                  key={id}
                >
                  <span>
                    {member
                      ? `${member.firstName} ${member.lastName}`
                      : "Member"}
                  </span>
                  {canManage ? (
                    <Button
                      variant="ghost"
                      disabled={pending}
                      onClick={() =>
                        void mutate(
                          () =>
                            runtime.api.removeDealTeamMember(
                              dealId,
                              team.teamId,
                              id,
                            ),
                          "Team assignment removed.",
                        )
                      }
                    >
                      Remove from team
                    </Button>
                  ) : null}
                </li>
              );
            })}
          </ul>
          {canManage ? (
            <>
              <form
                className="flex flex-wrap items-end gap-2"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (selected)
                    void mutate(
                      () =>
                        runtime.api.addDealTeamMember(
                          dealId,
                          team.teamId,
                          Number(selected),
                        ),
                      "Team member assigned.",
                    );
                }}
              >
                <SelectField
                  label={`Assign member to ${team.name}`}
                  value={selected || null}
                  onValueChange={setSelected}
                  disabled={
                    pending ||
                    members.every((member) =>
                      team.userIds.includes(member.userId),
                    )
                  }
                  className="min-w-48 flex-1"
                  placeholder="Select a member"
                  options={members
                    .filter((member) => !team.userIds.includes(member.userId))
                    .map((member) => ({
                      value: String(member.userId),
                      label: `${member.firstName} ${member.lastName}`,
                    }))}
                />
                <Button
                  variant="primary"
                  type="submit"
                  loading={pending}
                  disabled={!selected}
                >
                  Assign member
                </Button>
              </form>
              <form
                className="flex flex-wrap items-end gap-2"
                onSubmit={(e) => {
                  e.preventDefault();
                  void mutate(
                    () =>
                      runtime.api.renameDealTeam(dealId, team.teamId, {
                        name,
                        expectedRevision: team.revision,
                      }),
                    "Team renamed.",
                  );
                }}
              >
                <Field className="min-w-0 flex-1" disabled={pending}>
                  <FieldLabel htmlFor={fieldId}>Rename {team.name}</FieldLabel>
                  <Input
                    id={fieldId}
                    required
                    maxLength={100}
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                  />
                </Field>
                <Button disabled={pending || !name.trim()} type="submit">
                  Rename team
                </Button>
              </form>
              <Button
                variant="secondary"
                disabled={pending}
                onClick={() =>
                  void mutate(
                    () => runtime.api.deleteDealTeam(dealId, team.teamId),
                    "Team deleted. Deal membership is unchanged.",
                  )
                }
              >
                Delete team
              </Button>
            </>
          ) : null}
        </CollapsiblePanel>
      </Collapsible>
    </li>
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
