import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { WorkspaceLayout } from "./WorkspaceLayout";

describe("WorkspaceLayout", () => {
  it("renders the workspace content as an inset surface inside the shared shell", () => {
    const markup = renderToStaticMarkup(
      <WorkspaceLayout header={<span>Header</span>} sidebar={<aside>Sidebar</aside>}>
        <p>Content</p>
      </WorkspaceLayout>,
    );

    expect(markup).toContain("workspace-shell");
    expect(markup).toContain("workspace-main-surface");
    expect(markup).toContain("workspace-main-rail flex h-10");
    expect(markup.indexOf("Sidebar")).toBeLessThan(markup.indexOf("workspace-main-surface"));
    expect(markup.indexOf("Header")).toBeLessThan(markup.indexOf("Content"));
  });
});
