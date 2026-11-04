import { useState } from "react";
import { Search } from "lucide-react";
import { VaultTable } from "../components/hub/VaultTable";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { vaultTableRows } from "../fixtures/vault/items";

const vaultSections = [
  { label: "All items", type: null },
  { label: "Documents", type: "Document" },
  { label: "Insights", type: "Insight" },
  { label: "Links", type: "Link" },
  { label: "Prompts", type: "Prompt" },
  { label: "Notes", type: "Note" },
  { label: "Tasks", type: "Task" },
  { label: "Templates", type: "Template" },
  { label: "Global variables", type: "Variable" },
] as const;

const projects = [...new Set(vaultTableRows.map((row) => row.project))].sort();

export function VaultPage() {
  const [section, setSection] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [project, setProject] = useState("");
  const search = query.trim().toLocaleLowerCase();
  const rows = vaultTableRows.filter((row) =>
    (section === null || row.type === section) &&
    (!project || row.project === project) &&
    (!search || [row.name, row.description, row.project, row.practiceArea, row.creator, row.type]
      .some((value) => value?.toLocaleLowerCase().includes(search))),
  );
  const hasFilters = section !== null || query !== "" || project !== "";

  function clearFilters() {
    setSection(null);
    setQuery("");
    setProject("");
  }

  return (
    <WorkspaceHomeShell
      activeHomeSection="vault"
      contentClassName="py-6"
      header={<WorkspaceHeader title="Vault" />}
    >
      <div className="mx-auto w-full max-w-[1600px] px-4 pb-8 sm:px-8">
        <div className="mb-6 flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <p aria-hidden="true" className="type-h1 text-text-main">Vault</p>
            <p className="mt-2 w-full max-w-[36rem] text-[14px] leading-relaxed text-muted">
              Find documents, insights, and tasks across your workspace.
            </p>
          </div>
          <span className="rounded-md border border-outline-variant bg-surface-container-low px-2.5 py-1 text-[12px] text-muted">
            Sample data
          </span>
        </div>

        <div role="group" aria-label="Vault categories" className="mb-4 overflow-x-auto pb-2">
          <div className="flex min-w-max items-center gap-1">
            {vaultSections.map(({ label, type }) => {
              const count = vaultTableRows.filter((row) => type === null || row.type === type).length;
              return (
                <Button
                  key={label}
                  variant="ghost"
                  aria-pressed={section === type}
                  onClick={() => setSection(type)}
                  className={"h-11 gap-2 px-3 text-[13px] motion-reduce:transition-none " +
                    (section === type ? "bg-surface-container-high text-text-main" : "text-muted")}
                >
                  {label}
                  <span className="text-[11px] tabular-nums text-muted">{count}</span>
                </Button>
              );
            })}
          </div>
        </div>

        <section aria-label="Vault items" className="min-w-0">
          <div className="flex flex-wrap items-end gap-3 border-b border-outline-variant/70 pb-4">
            <div className="w-full sm:w-auto sm:flex-1 sm:max-w-[26rem]">
              <label htmlFor="vault-search" className="mb-1.5 block text-[12px] font-medium text-text-main">Search Vault</label>
              <div className="relative">
                <Search aria-hidden="true" className="pointer-events-none absolute left-3 top-3.5 h-4 w-4 text-muted" />
                <Input id="vault-search" type="search" placeholder="Search names, sources, or people" value={query} onChange={(event) => setQuery(event.target.value)} className="h-11 pl-9" />
              </div>
            </div>
            <div className="min-w-0 flex-1 sm:flex-none">
              <label htmlFor="vault-project" className="mb-1.5 block text-[12px] font-medium text-text-main">Project</label>
              <select id="vault-project" value={project} onChange={(event) => setProject(event.target.value)} className="h-11 w-full rounded-lg border border-outline-variant bg-surface-container-lowest px-3 text-[14px] text-text-main focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary sm:w-48">
                <option value="">All projects</option>
                {projects.map((name) => <option key={name} value={name}>{name}</option>)}
              </select>
            </div>
            {hasFilters && <Button variant="ghost" onClick={clearFilters} className="h-11 text-muted motion-reduce:transition-none">Clear filters</Button>}
            <p role="status" className="ml-auto pb-3 text-[12px] tabular-nums text-muted">{rows.length} {rows.length === 1 ? "item" : "items"}</p>
          </div>
          <VaultTable rows={rows} />
        </section>
      </div>
    </WorkspaceHomeShell>
  );
}
