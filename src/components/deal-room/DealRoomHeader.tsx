import { Icon } from "../ui/Icon";

type DealRoomOverviewProps = {
  description?: string;
  subtitle: string;
};

export function DealRoomHeader() {
  return (
    <header className="flex flex-wrap items-center gap-x-5 gap-y-2.5">
      <h1 className="text-[20px] font-semibold leading-6 tracking-[-0.015em] text-text-main [font-family:var(--font-heading)]">
        Deal Room
      </h1>
      <div aria-label="Deal resources" className="flex flex-wrap items-center gap-x-4 gap-y-1.5">
        <DealResource icon="doc" label="SOW" />
        <DealResource icon="doc" label="Fact Sheet" />
        <DealResource icon="sharepoint" label="SharePoint VDR" />
      </div>
    </header>
  );
}

export function DealRoomOverview({ description, subtitle }: DealRoomOverviewProps) {
  return (
    <header>
      <p className="type-subtle text-muted">{subtitle}</p>
      {description ? (
        <p className="mt-4 max-w-4xl text-[1.05rem] leading-8 text-text-main/90">{description}</p>
      ) : null}
    </header>
  );
}

function DealResource({ icon, label }: { icon: "doc" | "sharepoint"; label: string }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-[12px] font-semibold text-muted">
      <Icon className="h-3.5 w-3.5 text-primary" name={icon} />
      {label}
    </span>
  );
}
