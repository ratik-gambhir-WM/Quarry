import { useEffect, useState } from "react";
import { Navigate } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { WorkspaceCard } from "../components/hub/WorkspaceCard";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { Icon } from "../components/ui/Icon";
import { Skeleton } from "../components/ui/skeleton";
import type { WorkspaceAccountUser } from "../data/workspace";
import { useWorkspaceSession } from "../hooks/useWorkspaceSession";

type AccountLookupState =
  | { status: "loading" }
  | { message: string; status: "error" }
  | { status: "ready"; user: WorkspaceAccountUser | null };

export function AccountPage() {
  const { email } = useWorkspaceSession();
  const [lookup, setLookup] = useState<AccountLookupState>({ status: "loading" });

  useEffect(() => {
    if (!email) return;

    let active = true;
    setLookup({ status: "loading" });
    void runtime.api.getUserByEmail(email)
      .then((user) => {
        if (active) setLookup({ status: "ready", user });
      })
      .catch((error: unknown) => {
        if (active) {
          setLookup({
            message: error instanceof Error ? error.message : String(error),
            status: "error",
          });
        }
      });

    return () => {
      active = false;
    };
  }, [email]);

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
          {lookup.status === "loading" ? (
            <AccountInfoSkeleton />
          ) : lookup.status === "error" ? (
            <p className="text-[16px] font-medium text-error" role="alert">{lookup.message}</p>
          ) : lookup.user ? (
            <div className="grid gap-4 md:grid-cols-2">
              <AccountInfoItem label="Name" value={`${lookup.user.firstName} ${lookup.user.lastName}`} />
              <AccountInfoItem label="Email" value={lookup.user.email} />
              <AccountInfoItem label="Role" value={lookup.user.role} />
              <AccountInfoItem label="API key" value={maskApiKey(lookup.user.apiKey)} />
              <AccountInfoItem label="Created" value={formatDateTime(lookup.user.createdAt)} />
              <AccountInfoItem label="Updated" value={formatDateTime(lookup.user.updatedAt)} />
            </div>
          ) : (
            <p className="text-[16px] text-muted">No local account profile found for {email}.</p>
          )}
        </WorkspaceCard>
      </div>
    </WorkspaceHomeShell>
  );
}

function AccountInfoSkeleton() {
  return (
    <div aria-busy="true" aria-live="polite" role="status">
      <span className="sr-only">Loading account information</span>
      <div aria-hidden="true" className="grid gap-4 md:grid-cols-2">
        {Array.from({ length: 6 }, (_, index) => (
          <div className="space-y-3 rounded-[16px] border border-outline-variant/70 p-5" key={index}>
            <Skeleton className="h-3 w-20" />
            <Skeleton className="h-5 w-3/4" />
          </div>
        ))}
      </div>
    </div>
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
