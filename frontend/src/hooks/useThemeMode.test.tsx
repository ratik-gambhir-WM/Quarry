// @vitest-environment happy-dom

import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { ThemeModeProvider, useThemeMode } from "./useThemeMode";

function ThemeProbe() {
  const { setThemeMode, themeMode } = useThemeMode();

  return (
    <button onClick={() => setThemeMode("dark")} type="button">
      {themeMode}
    </button>
  );
}

describe("ThemeModeProvider", () => {
  beforeEach(() => {
    window.localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
  });

  it("normalizes stored and requested dark mode to the enabled light theme", () => {
    window.localStorage.setItem("quarry-theme-mode", "dark");

    render(
      <ThemeModeProvider>
        <ThemeProbe />
      </ThemeModeProvider>,
    );

    expect(screen.getByRole("button", { name: "slate-frost" })).toBeTruthy();
    expect(document.documentElement.dataset.theme).toBe("slate-frost");
    expect(window.localStorage.getItem("quarry-theme-mode")).toBe("slate-frost");

    fireEvent.click(screen.getByRole("button", { name: "slate-frost" }));

    expect(screen.getByRole("button", { name: "slate-frost" })).toBeTruthy();
    expect(document.documentElement.dataset.theme).toBe("slate-frost");
  });
});
