/* @vitest-environment happy-dom */

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DataRoomArcMenu } from "./DataRoomArcMenu";

afterEach(cleanup);

describe("DataRoomArcMenu", () => {
  it("starts closed but visible and can hide into and restore from the bookmark", async () => {
    const user = userEvent.setup();

    render(
      <DataRoomArcMenu
        documentSearch={{
          currentFileName: "Data Room",
          currentPageCount: 0,
          onActivateResult: vi.fn(),
        }}
      />,
    );

    expect(screen.getByRole("button", { name: "Open data room views" })).not.toBeNull();
    expect(screen.queryByRole("menu", { name: "Data room views" })).toBeNull();
    expect(screen.getByRole("button", { name: "Hide data room views" })).not.toBeNull();

    screen.getByRole("button", { name: "Open data room views" }).focus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("menu", { name: "Data room views" })).not.toBeNull();
    expect(
      screen.getByRole("menuitem", { name: "Search document" }).hasAttribute("disabled"),
    ).toBe(false);

    screen.getByRole("button", { name: "Close and retract shortcuts" }).focus();
    await user.keyboard("{Enter}");
    const hideButton = screen.getByRole("button", { name: "Hide data room views" });
    await user.click(hideButton);

    const showButton = screen.getByRole("button", { name: "Show data room views" });
    expect(showButton).toBe(document.activeElement);
    expect(screen.queryByRole("menu", { name: "Data room views" })).toBeNull();

    await user.click(showButton);
    await new Promise((resolve) => setTimeout(resolve, 250));

    expect(screen.getByRole("menu", { name: "Data room views" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Hide data room views" })).toBe(document.activeElement);
  });

  it("renders document search as an enabled arc action when configured", async () => {
    const onActivateResult = vi.fn();
    const user = userEvent.setup();

    render(
      <DataRoomArcMenu
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult,
        }}
      />,
    );

    screen.getByRole("button", { name: "Open data room views" }).focus();
    await user.keyboard("{Enter}");
    const searchAction = screen.getByRole("menuitem", { name: "Search document" });
    await user.click(searchAction);

    expect(screen.getByRole("dialog", { name: "Search Synthetic_Terms.pdf" })).not.toBeNull();
    expect(searchAction.hasAttribute("disabled")).toBe(false);
    await user.keyboard("{Escape}");
  });
});
