import {
  useEffect,
  useMemo,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { useLocation } from "react-router-dom";
import { useWorkspace } from "../../app/WorkspaceProvider";
import { createRequestResource } from "../../data/requests/requestResource";
import type { WorkspaceDeal } from "../../data/workspace";

/** Losing access unmounts previews, searches, uploads and session-only annotations. */
export function DealAccessBoundary({
  deal,
  children,
  onAccessDenied,
}: {
  deal: WorkspaceDeal;
  children: (deal: WorkspaceDeal) => ReactNode;
  onAccessDenied: () => void;
}) {
  const { loadDeal } = useWorkspace();
  const location = useLocation();
  const resource = useMemo(
    () =>
      createRequestResource(() => loadDeal(deal.room.id), {
        onError: (error) => {
          if (deniesAccess(error)) onAccessDenied();
        },
        refreshOnFocus: true,
        pollMs: 5000,
      }),
    [loadDeal, deal.room.id, onAccessDenied],
  );
  const access = useSyncExternalStore(
    resource.subscribe,
    resource.getSnapshot,
    resource.getSnapshot,
  );
  useEffect(() => {
    resource.revalidate();
  }, [location.key, resource]);
  if (access.status === "error" && deniesAccess(access.error)) {
    return null;
  }
  if (access.status === "error") {
    return (
      <section className="p-8" role="alert">
        <h1>Deal access unavailable</h1>
        <p>{access.message}</p>
        <button type="button" onClick={resource.refresh}>
          Retry
        </button>
      </section>
    );
  }
  if (access.status === "loading")
    return (
      <p role="status" className="p-8">
        Checking deal access…
      </p>
    );
  return (
    <div
      className="contents"
      key={`${deal.persisted?.accessRevision ?? 0}:${deal.persisted?.role ?? ""}`}
    >
      {children(deal)}
    </div>
  );
}
function deniesAccess(error: unknown) {
  if (typeof error !== "object" || error === null || !("status" in error))
    return false;
  return error.status === 403 || error.status === 404;
}
