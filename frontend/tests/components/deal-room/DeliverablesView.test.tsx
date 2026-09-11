// @vitest-environment happy-dom

import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { DeliverablesView } from "@/components/deal-room/DeliverablesView";

afterEach(cleanup);

describe("DeliverablesView", () => {
  it("renders the three slide empty states as labelled sections", () => {
    render(<DeliverablesView />);

    expect(screen.getByRole("heading", { level: 1, name: "Deliverables" })).not.toBeNull();

    const completedSection = screen.getByRole("region", { name: "Completed Slide(s)" });
    const inProgressSection = screen.getByRole("region", { name: "In-progress slides" });
    const templatesSection = screen.getByRole("region", { name: "Start from templates" });

    expect(within(completedSection).getByText("You have no completed slides")).not.toBeNull();
    expect(within(inProgressSection).getByText("You have no slides in progress")).not.toBeNull();
    expect(within(templatesSection).getByText("You have no templates")).not.toBeNull();

    const addTemplateButton = within(templatesSection).getByRole("button", { name: "Add template" });
    expect(addTemplateButton).toHaveProperty("disabled", true);
    expect(within(completedSection).queryByRole("button")).toBeNull();
    expect(within(inProgressSection).queryByRole("button")).toBeNull();
  });
});
