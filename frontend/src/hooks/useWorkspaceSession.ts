import { useUser } from "../auth/UserProvider";
import type { WorkspaceLocationState } from "../data/workspace";
export function useWorkspaceSession() {
  const { user } = useUser();
  const email = user?.email;
  return {
    email,
    navigationState: email
      ? ({ email } satisfies WorkspaceLocationState)
      : undefined,
  };
}
