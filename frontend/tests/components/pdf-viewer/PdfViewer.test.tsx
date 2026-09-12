// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { PdfViewer } from "@/components/pdf-viewer";

afterEach(cleanup);

describe("PdfViewer pending source", () => {
  it("keeps the viewer toolbar mounted above a document skeleton", () => {
    render(
      <PdfViewer
        ariaLabel="Pending document"
        enableDragDrop={false}
        pendingSource
        source={null}
      />,
    );

    expect(screen.getByRole("region", { name: "Pending document" })).toBeTruthy();
    expect(screen.getByRole("toolbar", { name: "PDF viewer controls" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Loading PDF");
    expect(document.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0);
    expect(screen.queryByText("No document loaded")).toBeNull();
  });
});
