import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import { WorkspaceAccountUser, WorkspaceLocationState } from "../../../data/workspace";
import { useThemeMode } from "../../../hooks/useThemeMode";
import { Icon } from "../../ui/Icon";

type ProfilePreferencesProps = {
  email?: string;
  navigationState?: WorkspaceLocationState;
};

export function ProfilePreferences({ email, navigationState }: ProfilePreferencesProps) {
  const navigate = useNavigate();
  const [accountError, setAccountError] = useState("");
  const [accountLoading, setAccountLoading] = useState(false);
  const { setThemeMode, themeMode } = useThemeMode();

  async function handleAccountInfo() {
    const workspaceEmail = email?.trim();

    if (!workspaceEmail) {
      navigate("/hub/account", { state: navigationState });
      return;
    }

    setAccountError("");
    setAccountLoading(true);

    try {
      const accountUser: WorkspaceAccountUser | null = await runtime.api.getUserByEmail(workspaceEmail);
      navigate("/hub/account", {
        state: {
          ...navigationState,
          accountLookupComplete: true,
          accountUser,
          email: workspaceEmail,
        } satisfies WorkspaceLocationState,
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setAccountError(message);
      navigate("/hub/account", {
        state: {
          ...navigationState,
          accountLookupComplete: true,
          accountLookupError: message,
          email: workspaceEmail,
        } satisfies WorkspaceLocationState,
      });
    } finally {
      setAccountLoading(false);
    }
  }

  return (
    <div
      className="absolute bottom-full left-0 z-20 mb-3 w-full rounded-2xl border border-outline-variant bg-white p-3 shadow-[0_18px_44px_rgba(7,1,84,0.12)]"
      role="menu"
    >
      <div className="space-y-3">
        <p className="px-2 text-[11px] font-bold uppercase tracking-[0.16em] text-muted">Theme</p>
        <div className="grid grid-cols-2 gap-1 rounded-full border border-outline-variant bg-surface-container-high p-1">
          <ThemeModeButton active={themeMode === "slate-frost"} label="Slate" onClick={() => setThemeMode("slate-frost")} />
          <ThemeModeButton active={themeMode === "dark"} label="Dark" onClick={() => setThemeMode("dark")} />
        </div>
        <div className="border-t border-outline-variant pt-3">
          <button
            className="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left text-[13px] font-semibold text-text-main transition hover:bg-surface-container-high focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-wait disabled:opacity-70"
            disabled={accountLoading}
            onClick={() => void handleAccountInfo()}
            role="menuitem"
            type="button"
          >
            <Icon className="h-4 w-4 text-muted" name="personSearch" />
            <span>{accountLoading ? "Loading account..." : "Account info"}</span>
          </button>
          {accountError ? <p className="mt-2 px-3 text-[11px] font-medium text-error">{accountError}</p> : null}
        </div>
      </div>
    </div>
  );
}

type ThemeModeButtonProps = {
  active: boolean;
  disabled?: boolean;
  label: string;
  onClick: () => void;
};

function ThemeModeButton({ active, disabled = false, label, onClick }: ThemeModeButtonProps) {
  return (
    <button
      aria-pressed={active}
      className={[
        "h-9 rounded-full text-[12px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-not-allowed disabled:opacity-45",
        active ? "bg-primary-container text-on-primary-container shadow-[0_6px_16px_rgba(7,1,84,0.16)]" : "text-primary hover:bg-white",
      ].join(" ")}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      {label}
    </button>
  );
}
