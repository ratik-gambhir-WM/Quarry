// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DataRoomExplorer } from "@/components/data-room/DataRoomExplorer";

afterEach(cleanup);

describe("DataRoomExplorer", () => {
  it("opens upload from the Saved documents plus without toggling the folder", async () => {
    const onUploadNewFile = vi.fn();
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <DataRoomExplorer
          dealName="Project Alpha"
          dealRoomPath="/hub/deals/project-alpha"
          nodes={[
            {
              defaultExpanded: true,
              id: "stored-documents",
              kind: "folder",
              name: "Saved documents",
              relativePath: "Saved documents",
            },
          ]}
          onSelectFile={vi.fn()}
          onUploadNewFile={onUploadNewFile}
        />
      </MemoryRouter>,
    );

    expect(screen.queryByRole("toolbar", { name: "Data room tools" })).toBeNull();
    const folder = screen.getByRole("button", { name: "Saved documents" });
    expect(folder.getAttribute("aria-expanded")).toBe("true");
    const plus = screen.getByRole("button", { name: "Upload files to Saved documents" });
    expect(folder.contains(plus)).toBe(false);
    await user.click(plus);
    expect(onUploadNewFile).toHaveBeenCalledTimes(1);
    expect(folder.getAttribute("aria-expanded")).toBe("true");
    await user.click(folder);
    expect(folder.getAttribute("aria-expanded")).toBe("false");
    await user.click(plus);
    expect(onUploadNewFile).toHaveBeenCalledTimes(2);
  });
});
