import { Icon } from "../../ui/Icon";

export function RepositorySearchEmptyState() {
  return (
    <div className="border border-dashed border-outline-variant bg-surface-container-lowest px-6 py-10 text-center">
      <Icon className="mx-auto h-6 w-6 text-muted" name="search" />
      <p className="mt-3 text-[15px] font-semibold text-text-main">No matching excerpts</p>
      <p className="mt-1 text-[13px] leading-5 text-muted">Try fewer terms or clear a filter.</p>
    </div>
  );
}
