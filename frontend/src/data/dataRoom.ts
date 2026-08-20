import { DealRoomData } from "./workspace";

export type DataRoomTreeNode = {
  children?: DataRoomTreeNode[];
  defaultExpanded?: boolean;
  error?: string;
  id: string;
  kind: "doc" | "folder" | "pdf" | "sheet";
  name: string;
  relativePath?: string;
};

export type DataRoomChip =
  | {
      body: string;
      category: string;
      id: string;
      tone: "accent" | "muted" | "primary";
      type: "text";
    }
  | {
      bars: number[];
      category: string;
      footer: string;
      id: string;
      tone: "muted" | "primary";
      type: "chart";
    }
  | {
      category: string;
      id: string;
      rows: Array<{ label: string; value: string }>;
      tone: "accent" | "primary";
      type: "metrics";
    };

export type EditorBlock =
  | { id: string; text: string; type: "paragraph" }
  | { id: string; text: string; type: "heading" }
  | { id: string; text: string; type: "quote" }
  | { id: string; type: "dropzone" }
  | {
      columns: Array<{
        items: string[];
        title: string;
        tone: "error" | "primary";
      }>;
      id: string;
      type: "callouts";
    };

export type DealDataRoomView = {
  chips: DataRoomChip[];
  editorBlocks: EditorBlock[];
  reportTitle: string;
  tree: DataRoomTreeNode[];
  versionLabel: string;
};

export function hasDataRoomFiles(nodes: DataRoomTreeNode[]): boolean {
  const pending = [...nodes];

  while (pending.length > 0) {
    const node = pending.pop();
    if (!node) continue;
    if (node.kind !== "folder") return true;
    if (node.children) pending.push(...node.children);
  }

  return false;
}

export function isUnconfiguredDataRoomError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes("no local data-room root is configured");
}

export function getDealDataRoomView(deal: DealRoomData): DealDataRoomView {
  return {
    chips: [
      {
        body: "Projected 12% growth in core regions based on H1 audit trails...",
        category: "EBITDA Analysis",
        id: "chip-ebitda",
        tone: "accent",
        type: "text",
      },
      {
        body: "3 pending IP litigations in the APAC region; low materiality but require monitoring.",
        category: "Legal Risk",
        id: "chip-legal",
        tone: "primary",
        type: "text",
      },
      {
        bars: [38, 61, 82, 44],
        category: "Market Share",
        footer: "Competitive Matrix - Q4",
        id: "chip-market",
        tone: "muted",
        type: "chart",
      },
      {
        category: "HR Metrics",
        id: "chip-hr",
        rows: [
          { label: "Retention", value: "92%" },
          { label: "Headcount", value: "450" },
        ],
        tone: "accent",
        type: "metrics",
      },
      {
        body: "GDPR audit completed; no major non-compliance issues noted.",
        category: "Compliance",
        id: "chip-compliance",
        tone: "primary",
        type: "text",
      },
    ],
    editorBlocks: [
      {
        id: "p-intro",
        text: `The acquisition of ${deal.name} represents a strategic pivot into the high-growth sustainability vertical. Our initial 48-hour sprint has uncovered several core value drivers that support the premium valuation suggested in the IOI.`,
        type: "paragraph",
      },
      {
        id: "h-financials",
        text: "1. Financial Performance & Forecasts",
        type: "heading",
      },
      {
        id: "p-financials",
        text: `${deal.name}'s recurring revenue model shows significant resilience. Over the past 36 months, churn rates have remained below 4%, significantly outperforming the industry average of 12% for comparable SaaS platforms.`,
        type: "paragraph",
      },
      { id: "dropzone-1", type: "dropzone" },
      {
        id: "p-cac",
        text: "However, we must reconcile the projected margins against the rising customer acquisition costs (CAC) observed in the late Q3 data room updates.",
        type: "paragraph",
      },
      {
        id: "quote-1",
        text: "The efficiency of the GTM motion remains the primary lever for the 24-month exit strategy. We need to validate the scalability of the current enterprise sales team.",
        type: "quote",
      },
      {
        id: "h-synergies",
        text: "2. Operational Synergies",
        type: "heading",
      },
      {
        id: "p-synergies",
        text: `By integrating ${deal.name}'s proprietary routing engine with our existing logistics network, we anticipate a 15% reduction in variable fulfillment costs within the first year of integration.`,
        type: "paragraph",
      },
      {
        columns: [
          {
            items: [
              "Automated compliance reporting",
              "Consolidated cloud infrastructure",
              "Talent pool cross-pollination",
            ],
            title: "Upside Potential",
            tone: "primary",
          },
          {
            items: [
              "Regional regulatory divergence",
              "Tech stack debt in legacy modules",
              "Integration bandwidth constraints",
            ],
            title: "Identified Risks",
            tone: "error",
          },
        ],
        id: "callouts",
        type: "callouts",
      },
      {
        id: "p-ip",
        text: "Further analysis is required on the intellectual property portfolio, specifically the patent filings in Germany and the UK which appear to be contested by a minor regional competitor.",
        type: "paragraph",
      },
    ],
    reportTitle: `${deal.name}: Initial Due Diligence`,
    tree: [
      {
        children: [
          {
            id: "general-info",
            kind: "folder",
            name: "1. General Info & Ownership",
          },
          {
            children: [
              {
                children: [
                  {
                    children: [
                      { id: "model-1", kind: "sheet", name: "2.1.1 Telluride - Model (Dec 2025).xlsx" },
                      { id: "model-2", kind: "sheet", name: "2.1.2 Telluride - June Variance Update.xlsx" },
                    ],
                    defaultExpanded: true,
                    id: "fin-model",
                    kind: "folder",
                    name: "2.1 Financial Model",
                  },
                  {
                    id: "balance-sheet",
                    kind: "folder",
                    name: "2.2 Balance Sheet Detail",
                  },
                  {
                    children: [
                      {
                        id: "capex",
                        kind: "folder",
                        name: "2.3.1 Capex - SW Development Costs",
                      },
                      {
                        id: "bank-rec",
                        kind: "folder",
                        name: "2.3.2 Bank Reconciliations",
                      },
                      {
                        id: "monthly-statements",
                        kind: "folder",
                        name: "2.3.3 Monthly Statements",
                      },
                      {
                        id: "transfer-pricing",
                        kind: "pdf",
                        name: "2.3.4 Transfer Pricing Docs - FY 2023-24.pdf",
                      },
                      {
                        id: "ebitda-adjustments",
                        kind: "sheet",
                        name: "2.3.5 EBITDA Adjustments.xlsx",
                      },
                      {
                        id: "shared-expenses",
                        kind: "sheet",
                        name: "2.3.6 Shared Expenses.xlsx",
                      },
                    ],
                    defaultExpanded: true,
                    id: "other-docs",
                    kind: "folder",
                    name: "2.3 Other Documents",
                  },
                ],
                defaultExpanded: true,
                id: "financial-detail-root",
                kind: "folder",
                name: "2. Financial Detail",
              },
            ],
            defaultExpanded: true,
            id: "vdr-root",
            kind: "folder",
            name: "VDR",
          },
        ],
        defaultExpanded: true,
        id: "project-root",
        kind: "folder",
        name: deal.name,
      },
    ],
    versionLabel: "Investment Report v2.1",
  };
}
