/* @vitest-environment happy-dom */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DataRoomQuickActions } from "@/components/data-room/DataRoomQuickActions";

afterEach(cleanup);

describe("DataRoomQuickActions", () => {
  it("renders evenly distributed Upload, Synthesis Canvas, Notes, and Search actions", () => {
    render(
      <DataRoomQuickActions
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        onUploadNewFile={vi.fn()}
      />,
    );

    const tools = screen.getByRole("toolbar", { name: "Data room tools" });
    expect(tools.querySelectorAll("button")).toHaveLength(4);
    expect(screen.getByRole("button", { name: "Upload files" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Open Synthesis Canvas" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Notes" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Search document" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Upload files" }).dataset.variant).toBe("ghost");
    expect(screen.getByRole("button", { name: "Open Synthesis Canvas" }).dataset.variant).toBe("ghost");
    expect(screen.getByRole("button", { name: "Notes" }).dataset.variant).toBe("ghost");
    expect(screen.getByRole("button", { name: "Search document" }).dataset.variant).toBe("ghost");
    expect(screen.queryByRole("menu", { name: "Data room views" })).toBeNull();
  });

  it("opens the existing file-upload flow from the Upload icon", async () => {
    const onUploadNewFile = vi.fn();
    const user = userEvent.setup();

    render(
      <DataRoomQuickActions
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        onUploadNewFile={onUploadNewFile}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Upload files" }));
    expect(onUploadNewFile).toHaveBeenCalledTimes(1);
  });

  it("opens the search panel directly and returns focus to its sidebar button", async () => {
    const user = userEvent.setup();

    render(
      <DataRoomQuickActions
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        onUploadNewFile={vi.fn()}
      />,
    );

    const searchTrigger = screen.getByRole("button", { name: "Search document" });
    await user.click(searchTrigger);

    expect(await screen.findByRole("dialog", { name: "Search Synthetic_Terms.pdf" })).not.toBeNull();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(searchTrigger).toBe(document.activeElement));
  });

  it("keeps the Synthesis Canvas draft while the panel is closed", async () => {
    const user = userEvent.setup();

    render(
      <DataRoomQuickActions
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        onUploadNewFile={vi.fn()}
      />,
    );

    const trigger = screen.getByRole("button", { name: "Open Synthesis Canvas" });
    await user.click(trigger);
    const notes = await screen.findByRole("textbox", { name: "Notes" });
    await user.type(notes, "Compare customer concentration.");
    await user.click(screen.getByRole("button", { name: "Close Synthesis Canvas" }));
    await waitFor(() => expect(trigger).toBe(document.activeElement));

    await user.click(trigger);
    expect(await screen.findByRole("textbox", { name: "Notes" })).toHaveProperty(
      "value",
      "Compare customer concentration.",
    );
  });
});
