import { createContext, useContext, useSyncExternalStore, type ReactNode } from "react";
import { runtime } from "@quarry/runtime";
import { sessionStore } from "./sessionStore";
import type { LoginInput, RegisterInput } from "../contracts/quarryApi";

const UserContext = createContext<ReturnType<typeof useSession> | null>(null);

function useSession() {
  const state = useSyncExternalStore(
    sessionStore.subscribe,
    sessionStore.getSnapshot,
    sessionStore.getSnapshot,
  );

  return {
    ...state,
    user: state.session?.user ?? null,
    userId: state.session?.user.id ?? null,
    token: state.session?.accessToken ?? null,
    expiresAt: state.session?.expiresAt ?? null,
    status: state.session ? "authenticated" as const : "anonymous" as const,
    async login(input: LoginInput, isActive = () => true) {
      const generation = sessionStore.getSnapshot().generation;
      const session = await runtime.api.login(input);
      if (isActive() && sessionStore.getSnapshot().generation === generation) {
        sessionStore.set(session);
      }
    },
    async register(input: RegisterInput, isActive = () => true) {
      const generation = sessionStore.getSnapshot().generation;
      const session = await runtime.api.register(input);
      if (isActive() && sessionStore.getSnapshot().generation === generation) {
        sessionStore.set(session);
      }
    },
    async logout() {
      const generation = sessionStore.getSnapshot().generation;
      let notice: string | undefined;
      try {
        await runtime.api.logout();
      } catch {
        notice = "Signed out on this device. Server sign-out could not be confirmed; the session will expire automatically.";
      }
      sessionStore.clearIfCurrent(generation, notice);
    },
  };
}

export function UserProvider({ children }: { children: ReactNode }) {
  const value = useSession();
  return <UserContext.Provider value={value}>{children}</UserContext.Provider>;
}

export function useUser() {
  const value = useContext(UserContext);
  if (!value) throw new Error("UserProvider is required.");
  return value;
}
