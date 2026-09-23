// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";
import { ThemeModeProvider } from "@/hooks/useThemeMode";
import { ProfilePreferences } from "@/components/hub/sidebar/ProfilePreferences";

describe("ProfilePreferences", () => {
  afterEach(cleanup);

  it("navigates to Account info immediately so the destination owns loading", async () => {
    const user = userEvent.setup();

    render(
      <MemoryRouter initialEntries={[{ pathname: "/hub", state: { email: "analyst@example.com" } }]}>
        <ThemeModeProvider>
          <ProfilePreferences
            email="analyst@example.com"
            navigationState={{ email: "analyst@example.com" }}
          />
          <LocationProbe />
        </ThemeModeProvider>
      </MemoryRouter>,
    );

    await user.click(screen.getByRole("menuitem", { name: "Account info" }));

    expect(screen.getByTestId("location").textContent).toBe("/hub/account");
  });

  it("keeps the account-only menu compact while the theme picker is unavailable", () => {
    render(
      <MemoryRouter>
        <ThemeModeProvider>
          <ProfilePreferences />
        </ThemeModeProvider>
      </MemoryRouter>,
    );

    const accountSection = screen.getByRole("menuitem", { name: "Account info" }).parentElement;

    expect(accountSection?.className).not.toContain("border-t");
    expect(accountSection?.className).not.toContain("pt-3");
  });
});

function LocationProbe() {
  const location = useLocation();
  return <output data-testid="location">{location.pathname}</output>;
}
