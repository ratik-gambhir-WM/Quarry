import { DealTimelineItem } from "../../data/workspace";
import {
  TimelineEntry,
  TimelineList,
  TimelineMarker,
  TimelinePanelAction,
  TimelinePanelHeader,
} from "./TimelinePanel";

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
      <TimelinePanelHeader title="Deal Activity" />

      <TimelineList>
        {items.map((item) => {
          const tone = toneMap[item.tone];

          return (
            <TimelineEntry
              key={item.id}
              marker={
                <TimelineMarker>
                  <span className={`h-3.5 w-3.5 rounded-full ${tone.dotClassName}`} />
                </TimelineMarker>
              }
            >
              <p className="text-[12px] font-medium text-muted">{item.timestamp}</p>
              <h3 className="text-[1.02rem] font-semibold text-text-main">{item.title}</h3>
              <div className={`rounded-[11px] border px-4 py-3 text-[1rem] leading-7 ${tone.detailClassName}`}>
                {item.detail}
              </div>
            </TimelineEntry>
          );
        })}
      </TimelineList>

      <TimelinePanelAction>View Full Log</TimelinePanelAction>
    </section>
  );
}
