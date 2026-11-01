import { useNavigate } from "react-router-dom";
import type { WorkspaceLocationState } from "../../../data/workspace";
import { DARK_THEME_ENABLED, useThemeMode } from "../../../hooks/useThemeMode";
import { Icon } from "../../ui/Icon";
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
} from "../../ui/dropdown-menu";

type ProfilePreferencesProps = {
  email?: string;
  navigationState?: WorkspaceLocationState;
};

export function ProfilePreferences({
  email,
  navigationState,
}: ProfilePreferencesProps) {
  const navigate = useNavigate();
  const { setThemeMode, themeMode } = useThemeMode();

  function handleAccountInfo() {
    const workspaceEmail = email?.trim();

    navigate("/hub/account", {
      state: workspaceEmail
        ? ({
            ...navigationState,
            email: workspaceEmail,
          } satisfies WorkspaceLocationState)
        : navigationState,
    });
  }

  return (
    <DropdownMenuContent side="top" align="start" sideOffset={12} className="w-64 rounded-xl p-2">
      {DARK_THEME_ENABLED ? (
        <>
          <DropdownMenuLabel>Theme</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={themeMode} onValueChange={(value) => {
            if (value === "dark" || value === "slate-frost") setThemeMode(value);
          }}>
            <DropdownMenuRadioItem value="slate-frost">Slate</DropdownMenuRadioItem>
            <DropdownMenuRadioItem value="dark">Dark</DropdownMenuRadioItem>
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
        </>
      ) : null}
      <DropdownMenuItem onSelect={handleAccountInfo} className="min-h-10 gap-3 px-3 text-[13px]">
        <Icon className="size-4 text-muted" name="personSearch" />
        Account info
      </DropdownMenuItem>
    </DropdownMenuContent>
  );
}
