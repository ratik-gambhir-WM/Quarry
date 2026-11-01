import {
  suggestedContentByTab,
  type SuggestedContentTab,
} from "../../../fixtures/hub/suggestedContent";
import { Icon } from "../../ui/Icon";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../../ui/tabs";

const tabs: SuggestedContentTab[] = ["Bookmarks", "Drafts", "Global Variables"];

export function SuggestedContentCard() {
  return (
    <section aria-labelledby="saved-work-title" className="flex flex-col gap-4 border-t border-outline-variant pt-6">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 id="saved-work-title" className="text-[15px] font-medium text-on-surface">Saved work</h2>
        <span className="rounded-md bg-surface-container-high px-2 py-1 text-[12px] text-muted">Sample content</span>
      </div>
      <p className="text-[13px] leading-5 text-muted">Preview bookmarks and drafts. These examples aren’t connected to your files.</p>
      <Tabs defaultValue="Bookmarks" className="gap-4">
        <TabsList aria-label="Content views" className="w-fit max-w-full gap-1 rounded-lg bg-surface-container-high p-1">
          {tabs.map((tab) => (
            <TabsTrigger key={tab} value={tab} className="min-h-10 rounded-md px-3 py-2 text-[13px] font-medium data-[state=active]:bg-surface-container-lowest data-[state=active]:shadow-sm">
              {tab === "Global Variables" ? "Global variables" : tab}
            </TabsTrigger>
          ))}
        </TabsList>
        {tabs.map((tab) => (
          <TabsContent key={tab} value={tab} className="space-y-6">
            {suggestedContentByTab[tab].map((group) => (
              <div key={group.label} className="space-y-2">
                <h3 className="text-[12px] font-medium text-muted">{group.label}</h3>
                <ul className="divide-y divide-outline-variant/50">
                  {group.items.map((item) => (
                    <li key={item.label} className="flex min-h-12 items-start gap-3 py-3 text-[13px] leading-5 text-on-surface">
                      <Icon className="mt-0.5 size-4 shrink-0 text-muted" name={item.icon} />
                      <span className="min-w-0 break-words">{item.label}</span>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </TabsContent>
        ))}
      </Tabs>
    </section>
  );
}
