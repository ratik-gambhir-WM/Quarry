// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import DocumentSearch, { filterDocumentSearchItems } from "./DocumentSearch";
import type { DocumentSearchItem } from "./documentSearchModel";

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

function Harness({ onSelect = vi.fn() }: { onSelect?: (item: DocumentSearchItem) => void }) {
  const [portalContainer, setPortalContainer] = useState<HTMLDivElement | null>(null);

  return (
    <div>
      <DocumentSearch
        buttonProps={{ "aria-label": "Search document" }}
        dialogTitle="Search Synthetic_Terms.pdf"
        items={items}
        onSelect={onSelect}
        portalContainer={portalContainer}
      />
      <div data-testid="preview-canvas">Mounted document canvas</div>
      <div data-testid="overlay-host" ref={setPortalContainer} />
    </div>
  );
}

afterEach(cleanup);

describe("DocumentSearch", () => {
  it("performs a simple local all-terms match", () => {
    expect(
      filterDocumentSearchItems(items, "synthetic liability").map((item) => item.id),
    ).toEqual(["current-document"]);
    expect(filterDocumentSearchItems(items, "missing term")).toEqual([]);
  });

  it("opens over the mounted canvas, focuses search, and restores trigger focus", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const trigger = screen.getByRole("button", { name: "Search document" });

    await user.click(trigger);

    const searchbox = screen.getByRole("searchbox", { name: "Search document" });
    expect(searchbox).toBe(document.activeElement);
    expect(
      screen.getByRole("dialog", { name: "Search Synthetic_Terms.pdf" }),
    ).toBeTruthy();
    expect(screen.getByTestId("preview-canvas")).toBeTruthy();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(trigger).toBe(document.activeElement);
  });

  it("highlights query terms and activates the selected mock result", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    const { container } = render(<Harness onSelect={onSelect} />);
    await user.click(screen.getByRole("button", { name: "Search document" }));
    await user.type(
      screen.getByRole("searchbox", { name: "Search document" }),
      "liability",
    );

    expect(container.querySelectorAll("mark").length).toBeGreaterThan(0);
    await user.keyboard("{Enter}");

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0]?.[0].id).toBe("current-document");
    expect(screen.queryByRole("dialog")).toBeNull();
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

    const result = screen.getByRole("option", { name: /Global Risk Assurance/ });
    expect(result.getAttribute("aria-disabled")).toBe("true");
    await user.keyboard("{Enter}");
    await user.click(result);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("handles an empty result set without an invalid active option", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(screen.getByRole("button", { name: "Search document" }));
    const searchbox = screen.getByRole("searchbox", { name: "Search document" });

    await user.type(searchbox, "no-such-synthetic-term");
    expect(screen.getByText(/No results for/)).toBeTruthy();
    expect(searchbox.hasAttribute("aria-activedescendant")).toBe(false);
    await user.keyboard("{ArrowDown}{ArrowUp}{Enter}");
    expect(screen.getByRole("dialog")).toBeTruthy();
  });
});
