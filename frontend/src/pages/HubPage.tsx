import { AiSearchCard } from "../components/hub/cards/AiSearchCard";
import { SuggestedContentCard } from "../components/hub/cards/SuggestedContentCard";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { hubAiSuggestions } from "../fixtures/hub/search";

export function HubPage() {
  return (
    <WorkspaceHomeShell header={<WorkspaceHeader title="Home" />}>
      <div className="mx-auto flex w-full max-w-[960px] flex-col gap-6 pb-10">
        <div className="space-y-2">
          <h2 className="type-h1 text-on-surface">Your workspace</h2>
          <p className="type-body-sm text-on-surface-variant">
            Pick up your work or start a conversation.
          </p>
        </div>

        <AiSearchCard suggestions={hubAiSuggestions} />

        <SuggestedContentCard />
      </div>
    </WorkspaceHomeShell>
  );
}
