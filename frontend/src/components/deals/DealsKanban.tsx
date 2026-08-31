import { useMemo } from "react";
import { buildDealKanbanLanes, type DealPortfolioView } from "../../data/dealsView";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { KanbanBoard } from "../kanban-board/kanban-board";
import type { AnyKanbanCardRenderer, KanbanData, KanbanItem } from "../kanban-board/types";
import { dealCardRenderer, type DealKanbanCardData } from "./DealKanbanCard";

type DealsKanbanProps = {
  deals: DealPortfolioView[];
  onOpenDeal: (dealId: string) => void;
};

const renderers: AnyKanbanCardRenderer[] = [dealCardRenderer];

export function DealsKanban({ deals, onOpenDeal }: DealsKanbanProps) {
  const data = useMemo<KanbanData>(() => ({
    columns: buildDealKanbanLanes(deals).map((lane) => ({
      id: lane.id,
      items: lane.deals.map((deal) => ({
        data: {
          closeDate: deal.closeDate,
          colorClassName: deal.colorClassName,
          id: deal.id,
          lifecycle: deal.lifecycle,
          name: deal.name,
          openQuestionCount: deal.openQuestionCount,
          sponsor: deal.sponsor,
          targetCompany: deal.targetCompany,
          type: deal.type,
        } satisfies DealKanbanCardData,
        id: `deal-${deal.transitionName}`,
        rendererId: dealCardRenderer.id,
      })),
      title: lane.title,
    })),
  }), [deals]);

  function handleItemClick(item: KanbanItem) {
    if (isDealKanbanCardData(item.data)) onOpenDeal(item.data.id);
  }

  return (
    <WorkspaceCard className="h-[min(650px,calc(100vh-17rem))] min-h-[420px] overflow-hidden" radius="small">
      <KanbanBoard
        aria-label="Deals by status"
        className="h-full"
        data={data}
        onItemClick={handleItemClick}
        readOnly
        renderers={renderers}
      />
    </WorkspaceCard>
  );
}

function isDealKanbanCardData(value: unknown): value is DealKanbanCardData {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<DealKanbanCardData>;
  return typeof candidate.id === "string" && typeof candidate.name === "string";
}
