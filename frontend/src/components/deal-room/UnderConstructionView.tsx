import { WorkspaceCard } from "../hub/WorkspaceCard";
import { Icon } from "../ui/Icon";

type UnderConstructionViewProps = {
  description: string;
  icon: "graph" | "grid" | "listAlt" | "person";
  title: string;
};

export function UnderConstructionView({ description, icon, title }: UnderConstructionViewProps) {
  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-2">
        <h1 className="type-display text-text-main">{title}</h1>
        <p className="type-subtle text-muted">{description}</p>
      </header>

      <WorkspaceCard
        className="flex min-h-[520px] items-center justify-center bg-surface-container-lowest p-8 text-center"
        radius="compact"
      >
        <div className="flex max-w-xl flex-col items-center gap-5">
          <div className="flex h-20 w-20 items-center justify-center rounded-full border border-outline-variant bg-surface-container-lowest text-primary shadow-[0_10px_24px_rgba(7,1,84,0.06)]">
            <Icon className="h-10 w-10" name={icon} />
          </div>

          <div className="space-y-3">
            <p className="text-[11px] font-bold uppercase tracking-[0.18em] text-muted">Template Page</p>
            <h2 className="type-h1 text-text-main">Under construction</h2>
            <p className="text-[1rem] leading-7 text-text-main/72">
              This workspace will be available here soon.
            </p>
          </div>
        </div>
      </WorkspaceCard>
    </div>
  );
}
