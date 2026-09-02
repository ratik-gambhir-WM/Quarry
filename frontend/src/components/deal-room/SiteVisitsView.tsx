import type { DealRoomData } from "../../data/workspace";
import { siteVisitFixtures } from "../../fixtures/deal-room/siteVisits";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { Icon } from "../ui/Icon";
import { DealRoomOverview } from "./DealRoomHeader";
import { DealRoomViewIntro } from "./DealRoomViewIntro";

type SiteVisitsViewProps = {
  deal: DealRoomData;
};

export function SiteVisitsView({ deal }: SiteVisitsViewProps) {
  return (
    <>
      <DealRoomOverview subtitle={`${deal.name} field diligence`} />

      <WorkspaceCard className="p-8" radius="compact">
        <DealRoomViewIntro
          action={
            <button className="inline-flex h-11 items-center justify-center gap-2 rounded-full bg-action px-5 text-[13px] font-bold text-on-action shadow-[0_10px_26px_rgba(7,1,84,0.2)] transition enabled:hover:bg-action-hover">
              <Icon className="h-4 w-4" name="plus" />
              New Visit
            </button>
          }
          className="flex-col gap-5 md:flex-row"
          description={<>Coordinate in-person reviews, owners, and open prep work for {deal.name}.</>}
          eyebrow="Site Visits"
          title="Field Diligence Plan"
        />

        <div className="mt-8 grid gap-4">
          {siteVisitFixtures.map((visit) => (
            <article
              className="grid gap-5 rounded-[16px] border border-outline-variant bg-white/62 p-5 shadow-[0_8px_20px_rgba(7,1,84,0.04)] md:grid-cols-[7rem_1fr_auto]"
              key={visit.location}
            >
              <div className="flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10 text-primary">
                <Icon className="h-7 w-7" name="person" />
              </div>
              <div className="min-w-0">
                <p className="text-[11px] font-bold uppercase tracking-[0.16em] text-muted">{visit.date}</p>
                <h2 className="mt-1 type-h3 text-text-main">{visit.location}</h2>
                <p className="mt-2 text-[14px] leading-6 text-text-main/78">{visit.summary}</p>
                <p className="mt-3 text-[12px] font-semibold text-muted">Owner: {visit.owner}</p>
              </div>
              <div className="flex items-start md:justify-end">
                <span className="rounded-full border border-outline-variant bg-white/72 px-3 py-1.5 text-[11px] font-bold uppercase tracking-[0.12em] text-primary">
                  {visit.status}
                </span>
              </div>
            </article>
          ))}
        </div>
      </WorkspaceCard>
    </>
  );
}
