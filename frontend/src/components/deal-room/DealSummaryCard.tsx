import type { DealRoomData } from "../../data/workspace";
import { Icon } from "../ui/Icon";

type DealSummaryCardProps = {
  deal: DealRoomData;
};

export function DealSummaryCard({ deal }: DealSummaryCardProps) {
  return (
    <section className="relative col-span-12 overflow-hidden p-8 lg:p-10">
      <div className="relative z-10 flex h-full flex-col gap-10">
        <div className="flex justify-end">
          <div className="flex h-16 w-16 shrink-0 items-center justify-center rounded-full border border-white/90 bg-white/80 text-primary shadow-[0_10px_24px_rgba(7,1,84,0.06)]">
            <Icon className="h-8 w-8" name="dataset" />
          </div>
        </div>

        <div className="grid gap-5 border-t border-white/60 pt-8 sm:grid-cols-3">
          {deal.metrics.map((metric) => (
            <div className="space-y-2" key={metric.label}>
              <p
                className={`text-[2.1rem] font-bold leading-none tracking-[-0.04em] [font-family:var(--font-heading)] ${
                  metric.tone === "error" ? "text-error" : "text-text-main"
                }`}
              >
                {metric.value}
              </p>
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-muted">{metric.label}</p>
            </div>
          ))}
        </div>

        <div className="border-t border-white/50 pt-8">
          <section className="space-y-4">
            <div className="flex items-center gap-3">
              <Icon className="h-6 w-6 text-primary" name="help" />
              <h3 className="type-h2 text-text-main">Key Questions</h3>
            </div>

            <div className="overflow-hidden rounded-[19px] border border-outline-variant bg-surface-container-lowest">
              <div className="workspace-scrollbar-hidden overflow-x-auto">
                <table className="w-full min-w-[860px] border-collapse text-left">
                  <colgroup>
                    <col className="w-3/5" />
                    <col className="w-2/5" />
                  </colgroup>
                  <thead>
                    <tr className="border-b border-outline-variant/70 bg-surface-container-low/70">
                      <QuestionTableHeading>Question</QuestionTableHeading>
                      <QuestionTableHeading>Answer</QuestionTableHeading>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-outline-variant/55">
                    {deal.keyQuestions.map((question, index) => (
                      <tr className="transition hover:bg-surface-container-low/55" key={`${question}-${index}`}>
                        <td className="px-4 py-4 align-top text-[13px] leading-6 text-text-main/82">{question}</td>
                        <td aria-label="No answer yet" className="px-4 py-4 align-top" />
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          </section>
        </div>
      </div>
    </section>
  );
}

function QuestionTableHeading({ children }: { children: string }) {
  return (
    <th className="px-4 py-3 text-[10px] font-bold uppercase tracking-[0.14em] text-muted" scope="col">
      {children}
    </th>
  );
}
