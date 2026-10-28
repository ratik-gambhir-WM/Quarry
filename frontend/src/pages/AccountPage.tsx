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
  return <WorkspaceHomeShell activeHomeSection="account" header={<WorkspaceHeader title="Account Info" />}>
    <WorkspaceCard className="p-8">
      <dl className="grid gap-4 md:grid-cols-2">{[
        ["Name", `${user.firstName} ${user.lastName}`], ["Username", user.username], ["Email", user.email], ["Job title", user.jobTitle ?? "Not provided"],
      ].map(([label, value]) => <div key={label}><dt className="type-label text-muted">{label}</dt><dd className="break-words type-body">{value}</dd></div>)}</dl>
      <div className="mt-lg"><QuarryButton disabled={pending} onClick={() => { setPending(true); void logout(); }}>{pending ? "Signing out…" : "Sign out"}</QuarryButton></div>
    </WorkspaceCard>
  </WorkspaceHomeShell>;
}
