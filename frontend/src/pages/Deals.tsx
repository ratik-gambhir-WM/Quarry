import {
  lazy,
  startTransition,
  Suspense,
  useDeferredValue,
  useMemo,
  useState,
  useTransition,
} from "react";
import { useNavigate } from "react-router-dom";
import { AddDealMenu } from "../components/deals/AddDealMenu";
import {
  DealLifecycleFilter,
  DealsSearch,
  DealsViewToggle,
  type DealsView,
} from "../components/deals/DealsHeaderControls";
import { DealsEmptyState, DealsTable } from "../components/deals/DealsTable";
import { WorkspaceCard } from "../components/hub/WorkspaceCard";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { useWorkspaceHomeDeals, WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { Skeleton } from "../components/ui/skeleton";
import { markViewTransitionType } from "../components/ui/ViewTransition";
import { filterDealPortfolioViews, getDealCounts, type DealScope } from "../data/dealsView";
import { getDealRoomPath, type WorkspaceLocationState } from "../data/workspace";
import { useWorkspaceSession } from "../hooks/useWorkspaceSession";

const loadDealsKanban = () => import("../components/deals/DealsKanban");
const DealsKanban = lazy(() =>
  loadDealsKanban().then((module) => ({ default: module.DealsKanban })),
);

const scopeLabels: Record<DealScope, string> = {
  all: "All deals",
  current: "Current deals",
  historic: "Historic deals",
};

export function Deals() {
  const { email, navigationState } = useWorkspaceSession();
  const [query, setQuery] = useState("");
  const [scope, setScope] = useState<DealScope>("all");
  const [view, setView] = useState<DealsView>("table");
  const [, startViewTransition] = useTransition();

  function changeView(nextView: DealsView) {
    if (nextView === view) return;
    startViewTransition(() => {
      markViewTransitionType("deals-view-change");
      setView(nextView);
    });
  }

  return (
    <WorkspaceHomeShell
      activeHomeSection="deals"
      header={
        <WorkspaceHeader
          actions={
            <>
              <DealLifecycleFilter
                onScopeChange={(nextScope) => startTransition(() => setScope(nextScope))}
                scope={scope}
              />
              <DealsViewToggle
                onPreloadKanban={() => {
                  void loadDealsKanban().catch(() => undefined);
                }}
                onViewChange={changeView}
                view={view}
              />
              <AddDealMenu email={email} />
            </>
          }
          afterTitle={<DealsSearch onQueryChange={setQuery} query={query} />}
          title="Deals"
        />
      }
    >
      <DealsContent
        navigationState={navigationState}
        onQueryChange={setQuery}
        onScopeChange={setScope}
        query={query}
        scope={scope}
        view={view}
      />
    </WorkspaceHomeShell>
  );
}

type DealsContentProps = {
  navigationState?: WorkspaceLocationState;
  onQueryChange: (query: string) => void;
  onScopeChange: (scope: DealScope) => void;
  query: string;
  scope: DealScope;
  view: DealsView;
};

function DealsContent({ navigationState, onQueryChange, onScopeChange, query, scope, view }: DealsContentProps) {
  const deals = useWorkspaceHomeDeals();
  const navigate = useNavigate();
  const deferredQuery = useDeferredValue(query);

  const counts = useMemo(() => getDealCounts(deals), [deals]);
  const visibleDeals = useMemo(
    () => filterDealPortfolioViews(deals, deferredQuery, scope),
    [deals, deferredQuery, scope],
  );

  function resetFilters() {
    onQueryChange("");
    startTransition(() => onScopeChange("all"));
  }

  return (
    <div className="mx-auto flex w-full max-w-[1240px] flex-col gap-5 pb-10">
      <div>
        <h2 className="text-[22px] font-medium leading-7 tracking-[-0.015em] text-text-main [font-family:var(--font-heading)]">
          {scopeLabels[scope]}
        </h2>
        <div aria-live="polite" className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-[12px] font-normal text-muted">
          <span><span className="font-medium tabular-nums text-text-main">{counts.total}</span> total</span>
          <span><span className="font-medium tabular-nums text-text-main">{counts.current}</span> current</span>
          <span><span className="font-medium tabular-nums text-text-main">{counts.historic}</span> historic</span>
        </div>
      </div>

      {visibleDeals.length === 0 ? (
        <DealsEmptyState onReset={resetFilters} />
      ) : view === "table" ? (
        <DealsTable deals={visibleDeals} navigationState={navigationState} onReset={resetFilters} />
      ) : (
        // Keep the lazy boundary unanimated so it does not suppress the per-deal shared morphs.
        <Suspense fallback={<KanbanFallback />}>
          <DealsKanban
            deals={visibleDeals}
            onOpenDeal={(dealId) => {
              navigate(getDealRoomPath(dealId), { state: navigationState });
            }}
          />
        </Suspense>
      )}
    </div>
  );
}

function KanbanFallback() {
  return (
    <WorkspaceCard
      aria-label="Loading kanban view"
      aria-live="polite"
      className="h-[min(650px,calc(100vh-17rem))] min-h-[420px] p-5"
      radius="small"
      role="status"
    >
      <span className="sr-only">Loading kanban view</span>
      <div aria-hidden="true" className="grid h-full gap-4 md:grid-cols-3">
        {Array.from({ length: 3 }, (_, columnIndex) => (
          <div className="flex flex-col gap-3" key={columnIndex}>
            <Skeleton className="h-5 w-28" />
            <Skeleton className="h-28 w-full rounded-xl" />
            <Skeleton className="h-28 w-full rounded-xl" />
          </div>
        ))}
      </div>
    </WorkspaceCard>
  );
}
