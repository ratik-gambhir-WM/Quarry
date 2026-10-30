import { useMemo } from "react";
import {
  buildDealKanbanLanes,
  type DealPortfolioView,
} from "../../data/dealsView";
import { KanbanBoard } from "../kanban-board/kanban-board";
import type {
  AnyKanbanCardRenderer,
  KanbanData,
  KanbanItem,
} from "../kanban-board/types";
import { dealCardRenderer, type DealKanbanCardData } from "./DealKanbanCard";
import "./deals-kanban.css";

type DealsKanbanProps = {
  deals: DealPortfolioView[];
  onOpenDeal: (dealId: string) => void;
};

const renderers: AnyKanbanCardRenderer[] = [dealCardRenderer];

export function DealsKanban({ deals, onOpenDeal }: DealsKanbanProps) {
  const data = useMemo<KanbanData>(
    () => ({
      columns: buildDealKanbanLanes(deals).map((lane) => ({
        id: lane.id,
        items: lane.deals.map((deal) => ({
          data: {
            closeDate: deal.closeDate,
            closeDateLabel: deal.closeDateLabel,
            engagement: deal.engagement,
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
    }),
    [deals],
  );

  function handleItemClick(item: KanbanItem) {
    if (isDealKanbanCardData(item.data)) onOpenDeal(item.data.id);
  }

  return (
    <div className="deals-kanban h-[calc(100dvh-9rem)] min-h-[420px] overflow-hidden">
      <KanbanBoard
        aria-label="Deals by status"
        className="h-full"
        data={data}
        onItemClick={handleItemClick}
        readOnly
        renderers={renderers}
      />
    </div>
  );
}

function isDealKanbanCardData(value: unknown): value is DealKanbanCardData {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<DealKanbanCardData>;
  return typeof candidate.id === "string" && typeof candidate.name === "string";
}
