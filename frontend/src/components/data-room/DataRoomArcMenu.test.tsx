/* @vitest-environment happy-dom */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DataRoomArcMenu } from "./DataRoomArcMenu";

vi.mock("motion/react", async (importOriginal) => {
  const motion = await importOriginal<typeof import("motion/react")>();

  return {
    ...motion,
    useReducedMotion: () => true,
  };
});

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

    const trigger = screen.getByRole("button", { name: "Open data room views" });
    expect(trigger.style.height).toBe("39.2px");
    expect(trigger.style.width).toBe("39.2px");
    expect(screen.queryByRole("menu", { name: "Data room views" })).toBeNull();
    const initialHideButton = screen.getByRole("button", { name: "Hide data room views" });
    expect(initialHideButton.classList.contains("h-[1.05rem]")).toBe(true);
    expect(initialHideButton.classList.contains("w-[2.8rem]")).toBe(true);

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
    expect(showButton.classList.contains("h-[2.45rem]")).toBe(true);
    expect(showButton.classList.contains("w-[4.9rem]")).toBe(true);
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
    await new Promise((resolve) => setTimeout(resolve, 150));
    const searchAction = screen.getByRole("menuitem", { name: "Search document" });
    await user.click(searchAction);
    await new Promise((resolve) => setTimeout(resolve, 150));

    expect(screen.getByRole("dialog", { name: "Search Synthetic_Terms.pdf" })).not.toBeNull();
    expect(
      document.querySelector('[data-slot="arc-menu"]')?.getAttribute("data-state"),
    ).toBe("closed");
    expect(searchAction.hasAttribute("disabled")).toBe(false);
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Open data room views" })).toBe(
        document.activeElement,
      ),
    );
  });

  it("opens an editable Synthesis Canvas panel and restores trigger focus on close", async () => {
    const onActivateResult = vi.fn();
    const user = userEvent.setup();
    const initialLocation = window.location.href;
    const expectedPanelX = Math.max(0, window.innerWidth - 360 - 72);
    const expectedPanelY = Math.max(0, window.innerHeight - 450 - 48);

    render(
      <DataRoomArcMenu
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult,
        }}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Open data room views" }));
    await new Promise((resolve) => setTimeout(resolve, 150));

    expect(screen.getByRole("menuitem", { name: "Data Room" }).hasAttribute("disabled")).toBe(
      true,
    );
    expect(
      screen.getByRole("menuitem", { name: "Diligence Graph" }).hasAttribute("disabled"),
    ).toBe(true);

    const synthesisAction = screen.getByRole("menuitem", { name: "Synthesis Canvas" });
    expect(synthesisAction.hasAttribute("disabled")).toBe(false);

    await user.click(synthesisAction);
    await new Promise((resolve) => setTimeout(resolve, 150));

    const synthesisPanel = await screen.findByRole("dialog", { name: "Synthesis Canvas" });
    expect(screen.queryByRole("menu", { name: "Data room views" })).toBeNull();
    const arcMenuTrigger = screen.getByRole("button", { name: "Open data room views" });
    expect(synthesisPanel.classList.contains("bg-surface-container-lowest")).toBe(true);
    expect(synthesisPanel.classList.contains("border-outline-variant")).toBe(true);
    expect(synthesisPanel.classList.contains("text-on-surface")).toBe(true);
    expect(synthesisPanel.classList.contains("outline-none")).toBe(true);
    const synthesisPanelHeader = synthesisPanel.querySelector(
      '[data-slot="floating-panel-header"]',
    );
    expect(synthesisPanelHeader?.classList.contains("bg-surface-container-low/70")).toBe(true);
    const positioner = document.querySelector<HTMLElement>(
      '[data-slot="floating-panel-positioner"]',
    );
    await waitFor(() => {
      expect(positioner?.style.getPropertyValue("--width")).toBe("360px");
      expect(positioner?.style.getPropertyValue("--height")).toBe("450px");
      expect(positioner?.style.getPropertyValue("--x")).toBe(`${expectedPanelX}px`);
      expect(positioner?.style.getPropertyValue("--y")).toBe(`${expectedPanelY}px`);
    });
    const notes = screen.getByRole("textbox", { name: "Notes" });
    expect(notes).toBe(document.activeElement);

    await user.type(notes, "Compare revenue quality with customer concentration.");
    expect(notes).toHaveProperty(
      "value",
      "Compare revenue quality with customer concentration.",
    );
    expect(onActivateResult).not.toHaveBeenCalled();
    expect(window.location.href).toBe(initialLocation);

    await user.click(screen.getByRole("button", { name: "Close Synthesis Canvas" }));
    await waitFor(() => expect(arcMenuTrigger).toBe(document.activeElement));
    expect(screen.queryByRole("dialog", { name: "Synthesis Canvas" })).toBeNull();

    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(screen.getByRole("menuitem", { name: "Synthesis Canvas" })).toBe(
        document.activeElement,
      ),
    );
    await new Promise((resolve) => setTimeout(resolve, 150));
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("dialog", { name: "Synthesis Canvas" })).not.toBeNull();
    expect(screen.queryByRole("menu", { name: "Data room views" })).toBeNull();
    expect(screen.getByRole("textbox", { name: "Notes" })).toHaveProperty(
      "value",
      "Compare revenue quality with customer concentration.",
    );

    await user.keyboard("{Escape}");
    await waitFor(() => expect(arcMenuTrigger).toBe(document.activeElement));
    expect(screen.queryByRole("dialog", { name: "Synthesis Canvas" })).toBeNull();
  });

  it("can reopen and retract the arc menu while the Synthesis Canvas panel remains open", async () => {
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

    await user.click(screen.getByRole("button", { name: "Open data room views" }));
    await new Promise((resolve) => setTimeout(resolve, 250));
    await user.click(screen.getByRole("menuitem", { name: "Synthesis Canvas" }));
    await new Promise((resolve) => setTimeout(resolve, 150));

    expect(screen.queryByRole("menu", { name: "Data room views" })).toBeNull();
    expect(screen.getByRole("dialog", { name: "Synthesis Canvas" })).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Open data room views" }));
    await new Promise((resolve) => setTimeout(resolve, 150));
    expect(screen.getByRole("menu", { name: "Data room views" })).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Close and retract shortcuts" }));
    expect(screen.queryByRole("menu", { name: "Data room views" })).toBeNull();
    expect(screen.getByRole("dialog", { name: "Synthesis Canvas" })).not.toBeNull();
    await new Promise((resolve) => setTimeout(resolve, 150));

    await user.click(screen.getByRole("button", { name: "Close Synthesis Canvas" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Open data room views" })).toBe(
        document.activeElement,
      ),
    );
    expect(screen.queryByRole("dialog", { name: "Synthesis Canvas" })).toBeNull();
  });
});
