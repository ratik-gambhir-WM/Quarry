// @vitest-environment happy-dom
import { createRef } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ScrollArea } from "@/components/ui/scroll-area";

afterEach(cleanup);

describe("shared ScrollArea", () => {
  it("preserves viewport attributes, events and both ref owners through cleanup", () => {
    const viewportRef = createRef<HTMLDivElement>();
    const adapterRef = vi.fn();
    const onScroll = vi.fn();
    const { unmount } = render(
      <ScrollArea viewportRef={adapterRef} viewportProps={{ ref: viewportRef, "aria-label": "Document viewport", onScroll }}>
        Document pages
      </ScrollArea>,
    );
    const viewport = screen.getByLabelText("Document viewport");
    expect(viewportRef.current).toBe(viewport);
    expect(adapterRef).toHaveBeenLastCalledWith(viewport);
    fireEvent.scroll(viewport);
    expect(onScroll).toHaveBeenCalledOnce();
    unmount();
    expect(viewportRef.current).toBeNull();
    expect(adapterRef).toHaveBeenLastCalledWith(null);
  });

  it("shares orientation behavior without replacing the viewport or its scroll position", () => {
    const viewportRef = createRef<HTMLDivElement>();
    const { container, rerender } = render(<ScrollArea orientation="vertical" viewportRef={viewportRef} viewportProps={{ "aria-label": "Pages" }}>Pages</ScrollArea>);
    const viewport = screen.getByLabelText("Pages");
    viewport.scrollTop = 120;
    const orientations = () => Array.from(container.querySelectorAll('[data-slot="scroll-area-scrollbar"]'), (bar) => bar.getAttribute("data-orientation"));
    expect(orientations()).toEqual(["vertical"]);
    rerender(<ScrollArea orientation="both" viewportRef={viewportRef} viewportProps={{ "aria-label": "Pages" }}>Pages</ScrollArea>);
    expect(orientations()).toEqual(["vertical", "horizontal"]);
    expect(viewportRef.current).toBe(viewport);
    expect(viewport.scrollTop).toBe(120);
  });
});
