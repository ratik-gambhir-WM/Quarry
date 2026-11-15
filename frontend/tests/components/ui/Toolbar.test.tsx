// @vitest-environment happy-dom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { Toolbar, ToolbarButton } from "@/components/ui/toolbar";

afterEach(cleanup);

it("composes real buttons and keeps disabled actions out of toolbar arrow navigation", async () => {
  const user = userEvent.setup();
  const onCopy = vi.fn();
  render(
    <Toolbar aria-label="Response actions">
      <ToolbarButton onClick={onCopy}>Copy</ToolbarButton>
      <ToolbarButton disabled>Retry</ToolbarButton>
      <ToolbarButton>Download</ToolbarButton>
    </Toolbar>,
  );
  expect(screen.getByRole("toolbar", { name: "Response actions" })).toBeTruthy();
  const copy = screen.getByRole("button", { name: "Copy" });
  await user.click(copy);
  expect(onCopy).toHaveBeenCalledOnce();
  await user.keyboard("{ArrowRight}");
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Download" }));
  expect(screen.getByRole("button", { name: "Retry" })).toHaveProperty("disabled", true);
});
