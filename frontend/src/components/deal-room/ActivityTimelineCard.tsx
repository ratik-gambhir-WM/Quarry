import { DealTimelineItem } from "../../data/workspace";
import { Icon } from "../ui/Icon";

type ActivityTimelineCardProps = {
  className?: string;
  items: DealTimelineItem[];
};

const toneMap: Record<DealTimelineItem["tone"], { detailClassName: string; dotClassName: string }> = {
  accent: {
    detailClassName: "border-white/70 bg-white/62 text-text-main/80",
    dotClassName: "bg-accent",
  },
  error: {
    detailClassName: "border-error/20 bg-error/5 text-error",
    dotClassName: "bg-error",
  },
  muted: {
    detailClassName: "border-transparent bg-transparent text-text-main/72",
    dotClassName: "bg-muted",
  },
  primary: {
    detailClassName: "border-transparent bg-transparent text-text-main/78",
    dotClassName: "bg-primary",
  },
};

export function ActivityTimelineCard({ className = "flex min-h-[540px] flex-col py-10", items }: ActivityTimelineCardProps) {
  return (
    <section className={className}>
      <div className="mb-7 flex items-center justify-between">
        <h2 className="type-h2 text-text-main">Deal Activity</h2>
        <button className="inline-flex h-10 w-10 items-center justify-center rounded-full transition hover:bg-white/50">
          <Icon className="h-5 w-5 text-muted" name="more" />
        </button>
      </div>

      <div className="workspace-scrollbar-hidden relative flex-1 overflow-y-auto pr-2">
        <div className="absolute bottom-6 left-4 top-4 w-px bg-muted/30" />

        <div className="relative z-10 space-y-7">
          {items.map((item) => {
            const tone = toneMap[item.tone];

            return (
              <article className="flex gap-4" key={item.id}>
                <div className="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-white/90 bg-white/85 shadow-[0_8px_18px_rgba(7,1,84,0.06)]">
                  <span className={`h-3.5 w-3.5 rounded-full ${tone.dotClassName}`} />
                </div>

                <div className="space-y-2 pb-1">
                  <p className="text-[12px] font-medium text-muted">{item.timestamp}</p>
                  <h3 className="text-[1.02rem] font-semibold text-text-main">{item.title}</h3>
                  <div className={`rounded-[11px] border px-4 py-3 text-[1rem] leading-7 ${tone.detailClassName}`}>
                    {item.detail}
                  </div>
                </div>
              </article>
            );
          })}
        </div>
      </div>

      <button className="mt-5 rounded-2xl border border-white/85 bg-white/55 px-4 py-3 text-[15px] font-semibold text-text-main transition hover:bg-white/75">
        View Full Log
      </button>
    </section>
  );
}
