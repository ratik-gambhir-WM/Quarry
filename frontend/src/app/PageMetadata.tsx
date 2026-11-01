import { matchPath, useLocation } from "react-router-dom";

const pageTitles: ReadonlyArray<readonly [string, string]> = [
  ["/", "Sign in"],
  ["/login", "Sign in"],
  ["/register", "Create an account"],
  ["/hub", "Home"],
  ["/hub/account", "Account"],
  ["/hub/vault", "Global vault"],
  ["/hub/initiatives/vault", "Vault"],
  ["/hub/assistant", "Assistant"],
  ["/hub/notebook/:noteId?", "Notebook"],
  ["/hub/logs", "Activity logs"],
  ["/hub/deals", "Deals"],
  ["/hub/deals/:dealId", "Deal overview"],
  ["/hub/deals/:dealId/activity", "Deal activity"],
  ["/hub/deals/:dealId/data-room", "Data room"],
  ["/hub/deals/:dealId/analysis", "Analysis"],
  ["/hub/deals/:dealId/deliverables", "Deliverables"],
  ["/hub/deals/:dealId/deliverables/templates", "Templates"],
  ["/hub/deals/:dealId/deliverables/templates/:templateId", "Template editor"],
];

export function PageMetadata() {
  const { pathname, search } = useLocation();
  const title = pageTitles.find(([path]) => matchPath(path, pathname))?.[1]
    ?? "Page not found";
  const isActivity = matchPath("/hub/deals/:dealId", pathname)
    && new URLSearchParams(search).get("section") === "activity";

  return <title>{`${isActivity ? "Deal activity" : title} — Quarry`}</title>;
}
