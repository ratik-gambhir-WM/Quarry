import { Suspense } from "react";
import type { ReactNode } from "react";
import { ViewTransition } from "../components/ui/ViewTransition";
import { WorkspacePageSkeleton } from "../components/ui/WorkspacePageSkeleton";
import { Skeleton } from "../components/ui/skeleton";

export function LazyPage({ children, label }: { children: ReactNode; label: string }) {
  return (
    <Suspense
      fallback={
        <ViewTransition default="none" exit="slide-down">
          <WorkspacePageSkeleton label={label} />
        </ViewTransition>
      }
    >
      <ViewTransition default="none" enter="slide-up">
        {children}
      </ViewTransition>
    </Suspense>
  );
}

export function LazyContent({ children, label }: { children: ReactNode; label: string }) {
  return (
    <Suspense
      fallback={
        <ViewTransition default="none" exit="slide-down">
          <div aria-busy="true" aria-live="polite" className="space-y-5 p-8" role="status">
            <span className="sr-only">{label}</span>
            <Skeleton className="h-8 w-64 max-w-full" />
            <Skeleton className="h-40 w-full" />
            <Skeleton className="h-40 w-full" />
          </div>
        </ViewTransition>
      }
    >
      <ViewTransition default="none" enter="slide-up">{children}</ViewTransition>
    </Suspense>
  );
}
