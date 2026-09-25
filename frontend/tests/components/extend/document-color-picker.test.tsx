// @vitest-environment happy-dom

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ColorPicker } from "@/components/extend/document-color-picker";
import { TooltipProvider } from "@/components/ui/tooltip";

describe("ColorPicker", () => {
  it("renders and opens within its popover root", async () => {
    const user = userEvent.setup();

    render(
      <TooltipProvider>
        <ColorPicker color="#facc15" label="Highlight color" onChange={() => {}} />
      </TooltipProvider>,
    );

    await user.click(screen.getByRole("button", { name: "Highlight color" }));

    expect(screen.getByRole("dialog")).toBeTruthy();
  });
});
