import { Skeleton } from "./skeleton";

type WorkspacePageSkeletonProps = {
  label?: string;
};

export function WorkspacePageSkeleton({ label = "Loading page" }: WorkspacePageSkeletonProps) {
  return (
    <div
      aria-busy="true"
      aria-live="polite"
      className="workspace-shell relative h-screen overflow-hidden text-on-surface"
      role="status"
    >
      <span className="sr-only">{label}</span>
      <div className="relative z-10 flex h-full min-h-0">
        <aside className="workspace-sidebar hidden h-full w-72 shrink-0 p-4 lg:block">
          <div className="flex h-full flex-col gap-6">
            <div className="flex h-8 items-center gap-3">
              <Skeleton className="h-8 w-8 rounded-lg" />
              <Skeleton className="h-4 w-28" />
            </div>
            <div className="space-y-3">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-5/6" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-4/5" />
            </div>
            <div className="mt-auto flex items-center gap-3 border-t border-outline-variant/70 pt-4">
              <Skeleton className="h-8 w-8 rounded-full" />
              <div className="flex-1 space-y-2">
                <Skeleton className="h-3 w-24" />
                <Skeleton className="h-2.5 w-32" />
              </div>
            </div>
          </div>
        </aside>

        <main className="workspace-main-surface m-2 flex min-w-0 flex-1 flex-col overflow-hidden lg:ml-0">
          <div className="workspace-main-rail flex h-10 shrink-0 items-center border-b px-6 sm:px-8">
            <Skeleton className="h-4 w-32" />
          </div>
          <div className="workspace-scrollbar-hidden min-h-0 flex-1 overflow-hidden px-8 py-8">
            <WorkspaceContentSkeleton />
          </div>
        </main>
      </div>
    </div>
  );
}

export function WorkspaceContentSkeleton() {
  return (
    <div className="mx-auto flex w-full max-w-[1100px] flex-col gap-6" aria-hidden="true">
      <div className="space-y-3">
        <Skeleton className="h-7 w-52" />
        <Skeleton className="h-4 w-full max-w-xl" />
      </div>
      <div className="grid gap-5 md:grid-cols-2">
        <Skeleton className="h-44 w-full rounded-2xl" />
        <Skeleton className="h-44 w-full rounded-2xl" />
      </div>
      <Skeleton className="h-56 w-full rounded-2xl" />
    </div>
  );
}
