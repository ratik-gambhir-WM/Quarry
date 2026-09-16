import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { WorkspaceLayout } from "@/components/hub/WorkspaceLayout";

describe("WorkspaceLayout", () => {
  it("renders the workspace content as an inset surface inside the shared shell", () => {
    const markup = renderToStaticMarkup(
      <WorkspaceLayout header={<span>Header</span>} sidebar={<aside>Sidebar</aside>}>
        <p>Content</p>
      </WorkspaceLayout>,
    );

    expect(markup).toContain("workspace-shell");
    expect(markup).toContain("workspace-shell-content");
    expect(markup).toContain("workspace-main-surface");
    expect(markup).toContain("data-workspace-primary-sidebar");
    expect(markup).toContain("workspace-main-rail flex h-10");
    expect(markup.indexOf("Sidebar")).toBeLessThan(markup.indexOf("workspace-main-surface"));
    expect(markup.indexOf("Header")).toBeLessThan(markup.indexOf("Content"));
  });

  it("keeps default scrolling intact and offers a route-body fill mode", () => {
    const scrolling = renderToStaticMarkup(
      <WorkspaceLayout header={null} sidebar={<aside>Sidebar</aside>}>
        Content
      </WorkspaceLayout>,
    );
    const fill = renderToStaticMarkup(
      <WorkspaceLayout contentMode="fill" header={null} sidebar={<aside>Sidebar</aside>}>
        Content
      </WorkspaceLayout>,
    );

    expect(scrolling).toContain("overflow-y-auto");
    expect(scrolling).toContain("px-8 py-8");
    expect(fill).toContain("min-h-0 flex-1 overflow-hidden p-0");
    expect(fill).not.toContain("overflow-y-auto");
  });
});
