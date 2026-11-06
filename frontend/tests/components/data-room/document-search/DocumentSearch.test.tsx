// @vitest-environment happy-dom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import DocumentSearch from "@/components/data-room/document-search/DocumentSearch";
import { filterDocumentSearchItems } from "@/components/data-room/document-search/documentSearchModel";
import type { DocumentSearchItem } from "@/components/data-room/document-search/documentSearchModel";

const items: DocumentSearchItem[] = [
  {
    id: "current-document",
    primaryText: "Synthetic Terms.pdf",
    secondaryText: "Liability coverage and environmental risk references.",
    tertiaryText: "Page 1 · Open page 1",
  },
  {
    disabledReason: "Preview navigation unavailable",
    id: "related-document",
    primaryText: "Global Risk Assurance 2023.pdf",
    secondaryText: "Environmental hazards and secondary liability coverage.",
    tertiaryText: "Page 42",
  },
];

function Harness({
  onSelect = vi.fn(),
  onSelectionFocus,
}: {
  onSelect?: (item: DocumentSearchItem) => void;
  onSelectionFocus?: () => void;
}) {
  const [boundaryElement, setBoundaryElement] = useState<HTMLDivElement | null>(
    null,
  );

  return (
    <div>
      <DocumentSearch
        buttonProps={{ "aria-label": "Search document" }}
        dialogTitle="Search Synthetic_Terms.pdf"
        items={items}
        onSelect={onSelect}
        onSelectionFocus={onSelectionFocus}
        boundaryElement={boundaryElement}
      />
      <button data-testid="preview-canvas">Mounted document canvas</button>
      <div data-testid="search-boundary" ref={setBoundaryElement} />
    </div>
  );
}

afterEach(cleanup);

describe("DocumentSearch", () => {
  it("performs a simple local all-terms match", () => {
    expect(
      filterDocumentSearchItems(items, "synthetic liability").map(
        (item) => item.id,
      ),
    ).toEqual(["current-document"]);
    expect(filterDocumentSearchItems(items, "missing term")).toEqual([]);
  });

  it("opens as a draggable panel over the mounted canvas and restores trigger focus", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const trigger = screen.getByRole("button", { name: "Search document" });

    await user.click(trigger);

    const searchbox = screen.getByRole("searchbox", {
      name: "Search document",
    });
    await waitFor(() => expect(searchbox).toBe(document.activeElement));
    const dialog = screen.getByRole("dialog", {
      name: "Search Synthetic_Terms.pdf",
    });
    expect(dialog).toBeTruthy();
    expect(dialog.classList.contains("bg-surface-container-lowest")).toBe(true);
    expect(
      dialog.classList.contains("shadow-[var(--theme-panel-shadow)]"),
    ).toBe(true);
    expect(
      dialog.querySelector('[data-slot="floating-panel-drag-trigger"]'),
    ).toBeTruthy();
    expect(document.querySelector('[class*="bg-black/50"]')).toBeNull();
    expect(screen.getByTestId("preview-canvas")).toBeTruthy();
    expect(dialog.querySelectorAll('[data-slot="floating-panel-resize-trigger"]').length).toBe(8);
    await user.click(screen.getByTestId("preview-canvas"));
    expect(document.activeElement).toBe(screen.getByTestId("preview-canvas"));
    expect(screen.getByRole("dialog")).toBeTruthy();
    await user.click(searchbox);

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await waitFor(() => expect(trigger).toBe(document.activeElement));
  });

  it("highlights query terms and activates the selected mock result", async () => {
    const onSelect = vi.fn();
    const onSelectionFocus = vi.fn();
    const user = userEvent.setup();
    render(<Harness onSelect={onSelect} onSelectionFocus={onSelectionFocus} />);
    await user.click(screen.getByRole("button", { name: "Search document" }));
    await user.type(
      screen.getByRole("searchbox", { name: "Search document" }),
      "liability",
    );

    expect(document.querySelectorAll("mark").length).toBeGreaterThan(0);
    await user.keyboard("{Enter}");

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0]?.[0].id).toBe("current-document");
    await waitFor(() => expect(onSelectionFocus).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("does not activate a mock result without a preview target", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    render(<Harness onSelect={onSelect} />);
    await user.click(screen.getByRole("button", { name: "Search document" }));
    await user.type(
      screen.getByRole("searchbox", { name: "Search document" }),
      "Global Risk",
    );

    const result = screen.getByRole("option", {
      name: /Global Risk Assurance/,
    });
    expect(result.getAttribute("aria-disabled")).toBe("true");
    await user.keyboard("{Enter}");
    await user.click(result);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("handles an empty result set without an invalid active option", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Search document" }));
    const searchbox = screen.getByRole("searchbox", {
      name: "Search document",
    });

    await user.type(searchbox, "no-such-synthetic-term");
    expect(screen.getByText(/No results for/)).toBeTruthy();
    expect(searchbox.hasAttribute("aria-activedescendant")).toBe(false);
    await user.keyboard("{ArrowDown}{ArrowUp}{Enter}");
    expect(screen.getByRole("dialog")).toBeTruthy();
  });
});

it("updates the preview with keyboard selection and clears back to guidance", async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(screen.getByRole("button", { name: "Search document" }));
  const input = screen.getByRole("searchbox", { name: "Search document" });
  await user.type(input, "liability");
  const preview = screen.getByRole("complementary", { name: "Preview", hidden: true });
  expect(preview.textContent).toContain("Synthetic Terms.pdf");
  await user.keyboard("{ArrowDown}");
  expect(preview.textContent).toContain("Global Risk Assurance 2023.pdf");
  expect(screen.getByRole("button", { name: "Open", hidden: true })).toHaveProperty("disabled", true);
  await user.click(screen.getByRole("button", { name: "Clear search" }));
  expect(document.activeElement).toBe(input);
  expect(input).toHaveProperty("value", "");
  expect(screen.queryAllByRole("option")).toHaveLength(0);
  expect(input.hasAttribute("aria-activedescendant")).toBe(false);
  expect(screen.getByText("Search files and excerpts to find a passage.")).toBeTruthy();
});
