import type { DealRoomData } from "../../data/workspace";
import { DataTableHeaderRow, DataTableHeading } from "../ui/DataTable";
import { Icon } from "../ui/Icon";

type DealSummaryCardProps = {
  deal: DealRoomData;
};

export function DealSummaryCard({ deal }: DealSummaryCardProps) {
  return (
    <section className="col-span-12 px-2 pb-8 pt-2 lg:pb-10">
      <div className="flex flex-col gap-8">
        <p className="max-w-[56rem] text-[1.05rem] leading-8 text-text-main/90">{deal.summary}</p>

        <div className="border-t border-outline-variant pt-8">
          <section className="space-y-4">
            <div className="flex items-center gap-3">
              <Icon className="h-6 w-6 text-primary" name="help" />
              <h3 className="type-h2 text-text-main">Key Questions</h3>
            </div>

            <div className="overflow-hidden rounded-[19px] border border-outline-variant bg-surface-container-lowest">
              <div>
                <table className="w-full table-fixed border-collapse text-left">
                  <colgroup>
                    <col className="w-1/2" />
                    <col className="w-1/2" />
                  </colgroup>
                  <thead>
                    <DataTableHeaderRow surface>
                      <DataTableHeading density="compact">Question</DataTableHeading>
                      <DataTableHeading className="border-l border-outline-variant/70" density="compact">
                        Answer
                      </DataTableHeading>
                    </DataTableHeaderRow>
                  </thead>
                  <tbody className="divide-y divide-outline-variant/55">
                    {deal.keyQuestions.map((question, index) => (
                      <tr className="transition hover:bg-surface-container-low/55" key={`${question}-${index}`}>
                        <td className="break-words px-4 py-4 align-top text-[13px] leading-6 text-text-main/82">
                          {question}
                        </td>
                        <td aria-label="No answer yet" className="border-l border-outline-variant/55 px-4 py-4 align-top" />
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
