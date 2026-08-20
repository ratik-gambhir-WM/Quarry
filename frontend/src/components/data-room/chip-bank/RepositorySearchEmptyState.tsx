import { Icon } from "../../ui/Icon";

export function RepositorySearchEmptyState() {
  return (
    <div className="rounded-lg border border-dashed border-outline-variant/70 bg-background px-6 py-10 text-center">
      <Icon className="mx-auto h-6 w-6 text-sidebar-muted" name="search" />
      <p className="mt-3 text-[13px] font-semibold text-sidebar-active">No matching excerpts</p>
      <p className="mt-1 text-[12px] leading-5 text-sidebar-muted">Try fewer terms or clear a filter.</p>
    </div>
  );
}
