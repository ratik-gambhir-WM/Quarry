import { Navigate, useLocation } from "react-router-dom";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { WorkspaceCard } from "../components/hub/WorkspaceCard";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { Icon } from "../components/ui/Icon";
import { WorkspaceLocationState } from "../data/workspace";
import { useWorkspaceSession } from "../hooks/useWorkspaceSession";

export function AccountPage() {
  const location = useLocation();
  const { email } = useWorkspaceSession();
  const state = (location.state ?? {}) as WorkspaceLocationState;
  const error = state.accountLookupError ?? "";
  const lookupComplete = Boolean(state.accountLookupComplete);
  const user = state.accountUser ?? null;

  if (!email) {
    return <Navigate replace to="/login" />;
  }

  return (
    <WorkspaceHomeShell activeHomeSection="account" header={<WorkspaceHeader title="Account Info" />}>
      <div className="mx-auto flex w-full max-w-[980px] flex-col gap-6 pb-10">
        <header className="space-y-3">
          <div className="inline-flex w-fit items-center gap-3 rounded-full border border-white/80 bg-white/70 px-4 py-2 text-[12px] font-semibold uppercase tracking-[0.18em] text-primary shadow-[0_8px_20px_rgba(7,1,84,0.05)]">
            <Icon className="h-4 w-4" name="personSearch" />
            Account
          </div>
          <p className="type-subtle max-w-3xl text-muted">Workspace profile details stored locally in Quarry.</p>
        </header>

        <WorkspaceCard className="p-8">
          {error ? (
            <p className="text-[16px] font-medium text-error">{error}</p>
          ) : user ? (
            <div className="grid gap-4 md:grid-cols-2">
              <AccountInfoItem label="Name" value={`${user.firstName} ${user.lastName}`} />
              <AccountInfoItem label="Email" value={user.email} />
              <AccountInfoItem label="Role" value={user.role} />
              <AccountInfoItem label="API key" value={maskApiKey(user.apiKey)} />
              <AccountInfoItem label="Created" value={formatDateTime(user.createdAt)} />
              <AccountInfoItem label="Updated" value={formatDateTime(user.updatedAt)} />
            </div>
          ) : (
            <p className="text-[16px] text-muted">
              {lookupComplete
                ? `No local account profile found for ${email}.`
                : "Open Account info from the profile menu to load your local account profile."}
            </p>
          )}
        </WorkspaceCard>
      </div>
    </WorkspaceHomeShell>
  );
}

type AccountInfoItemProps = {
  label: string;
  value: string;
};

function AccountInfoItem({ label, value }: AccountInfoItemProps) {
  return (
    <div className="rounded-[16px] border border-white/80 bg-white/70 p-5 shadow-[0_8px_20px_rgba(7,1,84,0.04)]">
      <p className="text-[11px] font-bold uppercase tracking-[0.18em] text-muted">{label}</p>
      <p className="mt-2 break-words text-[16px] font-semibold text-text-main">{value}</p>
    </div>
  );
}

function maskApiKey(apiKey: string) {
  if (!apiKey) {
    return "Not set";
  }

  if (apiKey.length <= 8) {
    return "••••";
  }

  return `${apiKey.slice(0, 3)}...${apiKey.slice(-4)}`;
}

function formatDateTime(value: string) {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return date.toLocaleString();
}
