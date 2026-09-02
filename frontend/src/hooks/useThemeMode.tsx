import { createContext, ReactNode, useCallback, useContext, useEffect, useMemo, useState } from "react";

export type ThemeMode = "slate-frost" | "dark";
export const DARK_THEME_ENABLED = false;

type ThemeModeContextValue = {
  setThemeMode: (themeMode: ThemeMode) => void;
  themeMode: ThemeMode;
};

const THEME_STORAGE_KEY = "quarry-theme-mode";
const THEME_COLORS: Record<ThemeMode, string> = {
  "slate-frost": "#f7f7f7",
  dark: "#070a1b",
};
const ThemeModeContext = createContext<ThemeModeContextValue | null>(null);

function getStoredThemeMode(): ThemeMode {
  if (typeof window === "undefined") {
    return "slate-frost";
  }

  const storedThemeMode = window.localStorage.getItem(THEME_STORAGE_KEY);
  return DARK_THEME_ENABLED && storedThemeMode === "dark" ? "dark" : "slate-frost";
}

type ThemeModeProviderProps = {
  children: ReactNode;
};

export function ThemeModeProvider({ children }: ThemeModeProviderProps) {
  const [themeMode, setThemeModeState] = useState<ThemeMode>(getStoredThemeMode);
  const setThemeMode = useCallback((nextThemeMode: ThemeMode) => {
    setThemeModeState(DARK_THEME_ENABLED ? nextThemeMode : "slate-frost");
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = themeMode;
    document.querySelector('meta[name="theme-color"]')?.setAttribute("content", THEME_COLORS[themeMode]);
    window.localStorage.setItem(THEME_STORAGE_KEY, themeMode);
  }, [themeMode]);

  const value = useMemo<ThemeModeContextValue>(() => ({ setThemeMode, themeMode }), [themeMode]);

  return <ThemeModeContext.Provider value={value}>{children}</ThemeModeContext.Provider>;
}

export function useThemeMode() {
  const context = useContext(ThemeModeContext);

  if (!context) {
    throw new Error("useThemeMode must be used within ThemeModeProvider");
  }

  return context;
}
