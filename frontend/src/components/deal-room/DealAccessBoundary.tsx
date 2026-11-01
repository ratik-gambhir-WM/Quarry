import { useEffect, useMemo, useSyncExternalStore, type ReactNode } from "react";
import { Navigate, useLocation } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import { useWorkspace } from "../../app/WorkspaceProvider";
import { createRequestResource } from "../../data/requests/requestResource";
import type { WorkspaceDeal } from "../../data/workspace";

/** Losing access unmounts previews, searches, uploads and session-only annotations. */
export function DealAccessBoundary({
  deal,
  children,
}: {
  deal: WorkspaceDeal;
  children: (deal: WorkspaceDeal) => ReactNode;
}) {
  const { cacheDealResult, forgetDeal } = useWorkspace();
  const location = useLocation();
  const resource = useMemo(
    () =>
      createRequestResource(() => runtime.api.getDeal(deal.room.id), {
        onSuccess: cacheDealResult,
        onError: (error) => {
          if (deniesAccess(error)) forgetDeal(deal.room.id);
        },
        refreshOnFocus: true,
        pollMs: 5000,
      }),
    [cacheDealResult, deal.room.id, forgetDeal],
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
    return (
      <Navigate
        replace
        to="/hub/deals"
        state={{ notice: "You no longer have access to this deal." }}
      />
    );
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
