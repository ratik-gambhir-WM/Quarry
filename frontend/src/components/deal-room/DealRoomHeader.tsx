import { useId } from "react";
import { Tabs, TabsList, TabsTrigger } from "../ui/tabs";

export type DealRoomOverviewSection =
  | "overview"
  | "activity"
  | "file-summary"
  | "evidence"
  | "findings"
  | "site-visits"
  | "open-items"
  | "settings";

type DealRoomHeaderProps = {
  activeSection: DealRoomOverviewSection;
  onActiveSectionChange: (section: DealRoomOverviewSection) => void;
};

const sections: ReadonlyArray<{
  enabled: boolean;
  label: string;
  value: DealRoomOverviewSection;
}> = [
  { enabled: true, label: "Overview", value: "overview" },
  { enabled: true, label: "Deal Activity", value: "activity" },
  { enabled: false, label: "Site Visits", value: "site-visits" },
  { enabled: true, label: "File Summary", value: "file-summary" },
  { enabled: false, label: "Evidence", value: "evidence" },
  { enabled: false, label: "Findings", value: "findings" },
  { enabled: false, label: "Open Items", value: "open-items" },
  { enabled: true, label: "Settings", value: "settings" },
];

export function getDealRoomSection(
  value: string | null,
): DealRoomOverviewSection {
  return (
    sections.find((section) => section.enabled && section.value === value)
      ?.value ?? "overview"
  );
}

export function DealRoomHeader({
  activeSection,
  onActiveSectionChange,
}: DealRoomHeaderProps) {
  const descriptionId = useId();

  return (
    <Tabs
      className="min-w-0 flex-1 self-stretch"
      onValueChange={(value) =>
        onActiveSectionChange(value as DealRoomOverviewSection)
      }
      value={activeSection}
    >
      <div className="workspace-scrollbar-hidden min-w-0 flex-1 overflow-x-auto">
        <TabsList
          aria-label="Deal room sections"
          className="h-full min-w-max gap-7"
        >
          {sections.map((section) => (
            <TabsTrigger
              aria-describedby={section.enabled ? undefined : descriptionId}
              aria-label={
                section.enabled
                  ? section.label
                  : `${section.label}, coming soon`
              }
              className="relative h-full border-b-2 border-transparent px-1 text-[12px] font-semibold data-[state=active]:border-primary data-[state=active]:text-primary"
              disabled={!section.enabled}
              key={section.value}
              title={section.enabled ? undefined : "Coming soon"}
              value={section.value}
            >
              {section.label}
            </TabsTrigger>
          ))}
        </TabsList>
      </div>
      <span className="sr-only" id={descriptionId}>
        This deal room section is coming soon and is not yet available.
      </span>
    </Tabs>
  );
}

export function DealRoomOverview({ subtitle }: { subtitle: string }) {
  return (
    <header>
      <p className="type-subtle text-muted">{subtitle}</p>
    </header>
  );
}
