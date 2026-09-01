/* @vitest-environment happy-dom */

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DataTableHeaderRow, DataTableHeading } from "./DataTable";
import { MetadataGrid, MetadataItem } from "./MetadataGrid";
import { ModalTextField } from "./ModalField";

afterEach(cleanup);

describe("reusable UI primitives", () => {
  it("preserves semantic table and description-list markup", () => {
    const { container } = render(
      <>
        <table>
          <thead>
            <DataTableHeaderRow>
              <DataTableHeading>Deal</DataTableHeading>
            </DataTableHeaderRow>
          </thead>
        </table>
        <MetadataGrid aria-label="Deal metadata">
          <MetadataItem label="Sponsor" value="Northwind" />
        </MetadataGrid>
      </>,
    );

    expect(screen.getByRole("columnheader", { name: "Deal" }).getAttribute("scope")).toBe("col");
    expect(container.querySelector("dl")).not.toBeNull();
    expect(container.querySelector("dt")?.textContent).toBe("Sponsor");
    expect(container.querySelector("dd")?.textContent).toBe("Northwind");
  });

  it("connects modal labels, optional state, and value changes", async () => {
    const onValueChange = vi.fn();
    const user = userEvent.setup();

    render(
      <ModalTextField
        id="sharepoint-url"
        label="SharePoint link"
        onValueChange={onValueChange}
        optional
        value=""
      />,
    );

    const input = screen.getByRole<HTMLInputElement>("textbox", { name: "SharePoint link Optional" });
    expect(input.required).toBe(false);
    expect(screen.getByText("Optional").textContent).toBe("Optional");

    await user.type(input, "https://example.com");
    expect(onValueChange).toHaveBeenCalled();
  });
});
