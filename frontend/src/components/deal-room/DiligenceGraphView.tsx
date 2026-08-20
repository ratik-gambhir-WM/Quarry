import type { DealRoomData } from "../../data/workspace";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { Icon } from "../ui/Icon";
import { DealRoomOverview } from "./DealRoomHeader";

type DiligenceGraphViewProps = {
  deal: DealRoomData;
};

export function DiligenceGraphView({ deal }: DiligenceGraphViewProps) {
  const graphNodes = [
    { label: deal.name, tone: "primary", x: "50%", y: "18%" },
    { label: "Financial Performance", tone: "accent", x: "20%", y: "48%" },
    { label: "Legal & Compliance", tone: "error", x: "50%", y: "54%" },
    { label: "Operations", tone: "muted", x: "80%", y: "48%" },
    { label: "Key Questions", tone: "primary", x: "50%", y: "82%" },
  ];

  return (
    <>
      <DealRoomOverview subtitle={`${deal.name} relationship map`} />

      <WorkspaceCard className="relative min-h-[680px] overflow-hidden p-8" radius="compact">
        <div className="flex items-start justify-between gap-6">
          <div>
            <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-muted">Diligence Graph</p>
            <h1 className="mt-3 type-display text-text-main">
              {deal.name} Knowledge Map
            </h1>
            <p className="mt-4 max-w-2xl text-[15px] leading-7 text-text-main/78">
              A working graph for connecting documents, findings, risks, and recommendations across the diligence process.
            </p>
          </div>
          <div className="hidden rounded-full border border-white/80 bg-white/72 px-4 py-2 text-[12px] font-semibold uppercase tracking-[0.14em] text-muted md:block">
            Prototype View
          </div>
        </div>

        <div className="relative mt-10 h-[460px] rounded-[19px] border border-outline-variant/70 bg-white/52">
          <svg aria-hidden="true" className="absolute inset-0 h-full w-full text-outline-variant" viewBox="0 0 100 100" preserveAspectRatio="none">
            <path className="fill-none stroke-current stroke-[0.35]" d="M50 18 L20 48 L50 82 L80 48 L50 18" />
            <path className="fill-none stroke-current stroke-[0.3]" d="M20 48 L50 54 L80 48 M50 18 L50 54 L50 82" />
          </svg>

          {graphNodes.map((node) => (
            <div
              className="absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center gap-2"
              key={node.label}
              style={{ left: node.x, top: node.y }}
            >
              <div
                className={`flex h-16 w-16 items-center justify-center rounded-full border-2 border-white text-white shadow-[0_14px_34px_rgba(7,1,84,0.16)] ${
                  node.tone === "error" ? "bg-error" : node.tone === "accent" ? "bg-accent" : node.tone === "muted" ? "bg-muted" : "bg-primary-container"
                }`}
              >
                <Icon className="h-7 w-7" name={node.label === deal.name ? "dataset" : "grid"} />
              </div>
              <span className="rounded-full border border-outline-variant bg-white/90 px-3 py-1 text-[12px] font-semibold text-text-main shadow-sm">
                {node.label}
              </span>
            </div>
          ))}
        </div>
      </WorkspaceCard>
    </>
  );
}
