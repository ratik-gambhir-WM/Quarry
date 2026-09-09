// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DataRoomDocumentSearch } from "./DataRoomDocumentSearch";

afterEach(cleanup);

describe("DataRoomDocumentSearch", () => {
  it("maps a selected result to a reusable activation callback", async () => {
    const onActivateResult = vi.fn();
    const user = userEvent.setup();

    render(
      <DataRoomDocumentSearch
        currentFileName="Synthetic_Terms.pdf"
        currentPageCount={3}
        onActivateResult={onActivateResult}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Search document" }));
    await user.type(screen.getByRole("searchbox", { name: "Search document" }), "Synthetic");
    await user.click(
      screen.getByRole("option", { name: /Synthetic Terms\.pdf.*Open page 1/ }),
    );

    expect(onActivateResult).toHaveBeenCalledTimes(1);
    expect(onActivateResult.mock.calls[0]?.[0]).toMatchObject({
      fileName: "Synthetic_Terms.pdf",
      target: { kind: "pdf-page", page: 1 },
    });
  });

  it("keeps unavailable cross-document results disabled", async () => {
    const onActivateResult = vi.fn();
    const user = userEvent.setup();

    render(
      <DataRoomDocumentSearch
        currentFileName="Synthetic_Terms.pdf"
        currentPageCount={3}
        onActivateResult={onActivateResult}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Search document" }));
    await user.type(screen.getByRole("searchbox", { name: "Search document" }), "Global Risk");
    await user.click(screen.getByRole("option", { name: /Global Risk Assurance/ }));

    expect(onActivateResult).not.toHaveBeenCalled();
  });
});
