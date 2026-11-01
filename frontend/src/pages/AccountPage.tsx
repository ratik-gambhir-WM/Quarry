import { useState } from "react";
import { useUser } from "../auth/UserProvider";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { WorkspaceCard } from "../components/hub/WorkspaceCard";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { QuarryButton } from "../components/ui/QuarryButton";
export function AccountPage() {
  const { user, logout } = useUser();
  const [pending, setPending] = useState(false);
  if (!user) return null;
  return (
    <WorkspaceHomeShell
      activeHomeSection="account"
      header={<WorkspaceHeader title="Account" />}
    >
      <WorkspaceCard className="mx-auto max-w-3xl p-5 sm:p-8" interactive={false}>
        <h2 className="type-h2 text-on-surface">Your profile</h2>
        <p className="mt-2 mb-6 type-body-sm text-on-surface-variant">Your account details and workspace identity.</p>
        <dl className="grid gap-4 md:grid-cols-2">
          {[
            ["Name", `${user.firstName} ${user.lastName}`],
            ["Username", user.username],
            ["Email", user.email],
            ["Job title", user.jobTitle ?? "Not provided"],
          ].map(([label, value]) => (
            <div key={label}>
              <dt className="type-label text-muted">{label}</dt>
              <dd className="mt-2 break-words type-body-sm text-on-surface">{value}</dd>
            </div>
          ))}
        </dl>
        <div className="mt-8 border-t border-outline-variant pt-6 sm:max-w-48">
          <QuarryButton
            disabled={pending}
            onClick={() => {
              setPending(true);
              void logout();
            }}
          >
            {pending ? "Signing out…" : "Sign out"}
          </QuarryButton>
        </div>
      </WorkspaceCard>
    </WorkspaceHomeShell>
  );
}
