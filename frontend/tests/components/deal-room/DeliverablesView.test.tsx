// @vitest-environment happy-dom

import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { DeliverablesView } from "@/components/deal-room/DeliverablesView";

afterEach(cleanup);

describe("DeliverablesView", () => {
  it("shows only completed and in-progress slide sections", () => {
    render(<DeliverablesView />);

    const completedSection = screen.getByRole("region", { name: "Completed Slide(s)" });
    const inProgressSection = screen.getByRole("region", { name: "In-progress slides" });

    expect(within(completedSection).getByText("You have no completed slides")).not.toBeNull();
    expect(within(inProgressSection).getByText("You have no slides in progress")).not.toBeNull();
    expect(completedSection.firstElementChild?.className).not.toContain("border-b");
    expect(inProgressSection.firstElementChild?.className).toContain("border-b");
    expect(completedSection.parentElement?.parentElement?.className).toContain("-mt-2");
    expect(screen.queryByText("Start from templates")).toBeNull();
    expect(screen.queryByRole("button", { name: "Add template" })).toBeNull();
  });
});
