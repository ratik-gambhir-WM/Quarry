// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Link, MemoryRouter } from "react-router-dom";
import { MobileWorkspaceNavigation } from "@/components/hub/MobileWorkspaceNavigation";
import { PageMetadata } from "@/app/PageMetadata";
import { MissingPageContent, NotFoundPage } from "@/pages/NotFoundPage";

afterEach(cleanup);

describe("Page polish", () => {
  it("keeps deal detail pages selected in the mobile workspace navigation", () => {
    render(<MemoryRouter initialEntries={["/hub/deals/deal-1/data-room"]}><MobileWorkspaceNavigation /></MemoryRouter>);
    expect(screen.getByRole("link", { name: "Deals" }).getAttribute("aria-current")).toBe("page");
    expect(screen.getByRole("link", { name: "Home" }).getAttribute("aria-current")).toBeNull();
    expect(screen.getAllByRole("link")).toHaveLength(6);
  });

  it("updates the browser title on client navigation, including deal activity", async () => {
    render(
      <MemoryRouter initialEntries={["/login"]}>
        <PageMetadata />
        <Link to="/hub/deals/deal-1?section=activity">Open activity</Link>
      </MemoryRouter>,
    );
    await waitFor(() => expect(document.title).toBe("Sign in — Quarry"));
    fireEvent.click(screen.getByRole("link", { name: "Open activity" }));
    await waitFor(() => expect(document.title).toBe("Deal activity — Quarry"));
  });

  it("offers a safe return from an unknown public page", () => {
    render(<MemoryRouter><NotFoundPage /></MemoryRouter>);
    expect(screen.getByRole("main").id).toBe("main-content");
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Page not found");
    expect(screen.getByRole("link", { name: "Back to sign in" }).getAttribute("href")).toBe("/login");
  });

  it("keeps recovery within the workspace for a signed-in missing page", () => {
    render(<MemoryRouter><MissingPageContent workspace /></MemoryRouter>);
    expect(screen.getByRole("link", { name: "Back to home" }).getAttribute("href")).toBe("/hub");
  });
});
