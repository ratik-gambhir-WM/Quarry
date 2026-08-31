import { AiSearchCard } from "../components/hub/cards/AiSearchCard";
import { SuggestedContentCard } from "../components/hub/cards/SuggestedContentCard";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";

const aiSuggestions = ['"Compare Q3 EBITDA across Alpha and Beta"', '"Summarize recent legal risks"'];

export function HubPage() {
  return (
    <WorkspaceHomeShell header={<WorkspaceHeader title="Quarry" />}>
      <div className="mx-auto flex w-full max-w-[960px] flex-col gap-6 pb-10">
        <p className="type-subtle text-muted">Portfolio Performance &amp; Strategic Initiatives</p>

        <AiSearchCard suggestions={aiSuggestions} />

        <SuggestedContentCard />
      </div>
    </WorkspaceHomeShell>
  );
}
