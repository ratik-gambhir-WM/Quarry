import { VaultTable } from "../components/hub/VaultTable";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { Icon } from "../components/ui/Icon";

const vaultSections = [
  { icon: "doc", label: "Documents" },
  { icon: "openInNew", label: "Links" },
  { icon: "sparkles", label: "Prompts" },
  { icon: "listAlt", label: "Notes" },
  { icon: "checkCircle", label: "Tasks" },
] as const;

export function VaultPage() {
  return (
    <WorkspaceHomeShell activeHomeSection="vault" header={<WorkspaceHeader title="Vault" />}>
      <div className="mx-auto w-full max-w-[1120px] pb-10">
        <div className="border-b border-outline-variant/70 pb-5">
          <p aria-hidden="true" className="type-h1 text-text-main">
            Vault
          </p>

          <nav aria-label="Vault sections" className="mt-6 overflow-x-auto">
            <ul className="flex min-w-max items-center gap-1" role="list">
              {vaultSections.map(({ icon, label }, index) => (
                <li
                  className={
                    "flex items-center gap-2 rounded-lg px-4 py-3 text-[14px] font-medium " +
                    (index === 0 ? "bg-surface-container text-text-main" : "text-muted")
                  }
                  data-active={index === 0 ? "true" : undefined}
                  key={label}
                >
                  <Icon className="h-4 w-4 shrink-0" name={icon} />
                  <span>{label}</span>
                </li>
              ))}
            </ul>
          </nav>
        </div>

        <div className="mt-8">
          <VaultTable />
        </div>
      </div>
    </WorkspaceHomeShell>
  );
}
