import { useEffect } from "react";
import { useLocation } from "react-router-dom";
import { WorkspaceLocationState } from "../data/workspace";

const workspaceEmailStorageKey = "quarry.workspace.email";

export function persistWorkspaceEmail(email: string) {
  if (typeof window === "undefined" || !email) {
    return;
  }

  window.sessionStorage.setItem(workspaceEmailStorageKey, email);
}

function readWorkspaceEmail() {
  if (typeof window === "undefined") {
    return undefined;
  }

  return window.sessionStorage.getItem(workspaceEmailStorageKey) ?? undefined;
}

export function useWorkspaceSession() {
  const location = useLocation();
  const state = (location.state ?? {}) as WorkspaceLocationState;
  const email = state.email ?? readWorkspaceEmail();

  useEffect(() => {
    if (state.email) {
      persistWorkspaceEmail(state.email);
    }
  }, [state.email]);

  return {
    email,
    navigationState: email ? ({ email } satisfies WorkspaceLocationState) : undefined,
  };
}
