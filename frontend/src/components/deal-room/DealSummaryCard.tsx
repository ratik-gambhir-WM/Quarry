import { useMemo } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { useTable } from "@tanstack/react-table";
import type { DealResource, DealRoomData } from "../../data/workspace";
import {
  DataGrid,
  DataGridContainer,
  dataGridFeatures,
  type DataGridFeatures,
} from "../reui/data-grid/data-grid";
import { DataGridColumnHeader } from "../reui/data-grid/data-grid-column-header";
import { DataGridScrollArea } from "../reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "../reui/data-grid/data-grid-table";
import {
  quarryDataGridHeaderClassName,
  quarryDataGridTableClassNames,
} from "../reui/data-grid/quarry-data-grid";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { Icon } from "../ui/Icon";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Separator } from "../ui/separator";

type DealSummaryCardProps = {
  deal: DealRoomData;
};

export function DealSummaryCard({ deal }: DealSummaryCardProps) {
  return (
    <div className="grid grid-cols-12 gap-4 lg:gap-6">
      <WorkspaceCard
        aria-labelledby="deal-overview-heading"
        className="col-span-12 flex min-h-64 flex-col p-6 sm:p-7 lg:col-span-7"
        interactive={false}
        surface="chrome"
      >
        <div className="flex flex-col gap-5">
          <div>
            <p className="type-label uppercase tracking-[0.14em] text-muted">Due Diligence Overview</p>
            <h1 className="type-display mt-2 break-words text-text-main" id="deal-overview-heading">
              {deal.name}
            </h1>
          </div>
          <div className="flex flex-wrap gap-2">
            <Badge variant="default">{deal.stageLabel}</Badge>
            <Badge variant="secondary">{deal.phaseLabel}</Badge>
            <Badge variant="outline">{deal.sectorLabel}</Badge>
          </div>
          <Separator className="bg-outline-variant/70" />
          <p className="max-w-[58rem] text-[14px] leading-7 text-text-main/86 sm:text-[15px]">
            {deal.summary}
          </p>
        </div>
      </WorkspaceCard>

      <WorkspaceCard
        aria-labelledby="deal-resources-heading"
        className="col-span-12 p-5 sm:p-6 lg:col-span-5"
        interactive={false}
        surface="chrome"
      >
        <div className="mb-4 flex items-center gap-2.5">
          <span className="flex size-9 items-center justify-center rounded-xl bg-primary/8 text-primary">
            <Icon className="size-4.5" name="folderOpen" />
          </span>
          <div>
            <h2 className="type-h3 text-text-main" id="deal-resources-heading">Deal Resources</h2>
            <p className="mt-0.5 text-[11px] text-muted">Source documents and connected rooms</p>
          </div>
        </div>
        <div className="space-y-2" role="list">
          {deal.resources.map((resource) => <ResourceRow key={resource.id} resource={resource} />)}
        </div>
      </WorkspaceCard>

      <WorkspaceCard
        aria-labelledby="key-questions-heading"
        className="col-span-12 overflow-hidden"
        interactive={false}
        surface="chrome"
      >
        <div className="flex items-center px-5 pb-4 pt-5 sm:px-6 sm:pt-6">
          <div>
            <h2 className="type-h3 text-text-main" id="key-questions-heading">Key Questions</h2>
            <p className="mt-0.5 text-[11px] text-muted">Questions extracted from submitted SOW</p>
          </div>
        </div>
        <KeyQuestionsGrid questions={deal.keyQuestions} />
      </WorkspaceCard>
    </div>
  );
}

function ResourceRow({ resource }: { resource: DealResource }) {
  const content = (
    <>
      <span className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-surface-container-lowest text-primary">
        <Icon className="size-4" name={resource.id === "sharepoint" ? "sharepoint" : "doc"} />
      </span>
      <span className="min-w-0 flex-1 text-left">
        <span className="flex min-w-0 items-center justify-between gap-3">
          <span className="min-w-0 break-words text-[13px] font-semibold text-text-main">{resource.label}</span>
          <Badge className="shrink-0" variant={resource.availability === "available" ? "secondary" : "outline"}>
            {resource.availability === "available"
              ? resource.href ? "Available" : "Submitted"
              : resource.availability === "coming-soon"
                ? "Coming soon"
                : "Unavailable"}
          </Badge>
        </span>
        {resource.sourceName ? (
          <span className="mt-0.5 block break-words text-[11px] font-normal text-muted">{resource.sourceName}</span>
        ) : null}
      </span>
      {resource.href ? <Icon className="size-4 shrink-0 text-muted" name="openInNew" /> : null}
    </>
  );

  if (resource.href) {
    return (
      <div role="listitem">
        <Button asChild className="h-auto w-full justify-start gap-3 rounded-xl px-3 py-3" variant="ghost">
          <a
            aria-label={`Open ${resource.label} in a new tab`}
            href={resource.href}
            rel="noopener noreferrer"
            target="_blank"
          >
            {content}
          </a>
        </Button>
      </div>
    );
  }

  return (
    <div
      aria-disabled="true"
      className="flex w-full items-center gap-3 rounded-xl px-3 py-3 opacity-70"
      role="listitem"
    >
      {content}
    </div>
  );
}

type KeyQuestionRow = {
  answer: string;
  id: string;
  question: string;
};

function KeyQuestionsGrid({ questions }: { questions: readonly string[] }) {
  const rows = useMemo<KeyQuestionRow[]>(
    () => questions.map((question, index) => ({ answer: "", id: `question-${index}`, question })),
    [questions],
  );
  const columns = useMemo<ColumnDef<DataGridFeatures, KeyQuestionRow>[]>(
    () => [
      {
        accessorKey: "question",
        enableSorting: false,
        header: ({ column }) => (
          <DataGridColumnHeader className={quarryDataGridHeaderClassName} column={column} title="Question" />
        ),
        cell: ({ row }) => <p className="whitespace-normal break-words text-[13px] leading-6">{row.original.question}</p>,
        size: 520,
        meta: { fillWidth: true, headerTitle: "Question" },
      },
      {
        accessorKey: "answer",
        enableSorting: false,
        header: ({ column }) => (
          <DataGridColumnHeader className={quarryDataGridHeaderClassName} column={column} title="Answer" />
        ),
        cell: () => <span aria-label="No answer yet" className="text-muted">—</span>,
        size: 320,
        meta: { headerTitle: "Answer" },
      },
    ],
    [],
  );
  const table = useTable({
    columns,
    data: rows,
    features: dataGridFeatures,
    getRowId: (row) => row.id,
    state: { pagination: { pageIndex: 0, pageSize: Math.max(rows.length, 1) } },
  });

  return (
    <DataGrid
      emptyMessage="No key questions were extracted from the submitted source files."
      recordCount={rows.length}
      table={table}
      tableClassNames={quarryDataGridTableClassNames}
      tableLayout={{ cellBorder: true, dense: true, headerBackground: false, headerBorder: true, rowBorder: true, width: "fixed" }}
    >
      <DataGridContainer className="border-t border-outline-variant/70 bg-surface-container-lowest">
        <DataGridScrollArea orientation="horizontal">
          <DataGridTable />
        </DataGridScrollArea>
      </DataGridContainer>
    </DataGrid>
  );
}
