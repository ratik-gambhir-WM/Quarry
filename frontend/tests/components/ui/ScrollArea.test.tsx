// @vitest-environment happy-dom
import { createRef } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ScrollArea } from "@/components/ui/scroll-area";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("shared ScrollArea", () => {
  it.each(["vertical", "horizontal"] as const)(
    "updates %s overflow and keyboard access when only the content resizes",
    async (orientation) => {
      let contentSize = 50;
      const observers = new Map<ResizeObserver, {
        callback: ResizeObserverCallback;
        targets: Set<Element>;
      }>();
      vi.stubGlobal("ResizeObserver", class implements ResizeObserver {
        constructor(callback: ResizeObserverCallback) {
          observers.set(this, { callback, targets: new Set() });
        }
        observe(target: Element) { observers.get(this)?.targets.add(target); }
        unobserve(target: Element) { observers.get(this)?.targets.delete(target); }
        disconnect() { observers.delete(this); }
      });
      vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(100);
      vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(100);
      vi.spyOn(HTMLElement.prototype, "scrollHeight", "get")
        .mockImplementation(() => orientation === "vertical" ? contentSize : 100);
      vi.spyOn(HTMLElement.prototype, "scrollWidth", "get")
        .mockImplementation(() => orientation === "horizontal" ? contentSize : 100);

      render(<ScrollArea orientation={orientation} scrollFade viewportProps={{ "aria-label": "Resizable pages" }}>
        <div>Document pages</div>
      </ScrollArea>);
      const viewport = screen.getByLabelText("Resizable pages");
      // Deliver initial observations before simulating a content-only resize.
      await act(async () => {
        observers.forEach(({ callback }, observer) => callback([], observer));
        await Promise.resolve();
      });
      expect(viewport.tabIndex).toBe(-1);

      for (const size of [300, 50]) {
        contentSize = size;
        await act(async () => {
          observers.forEach(({ callback, targets }, observer) => {
            if ([...targets].some((target) => target !== viewport && viewport.contains(target))) {
              callback([], observer);
            }
          });
        });
        const overflowing = size > 100;
        const axis = orientation === "vertical" ? "y" : "x";
        expect(viewport.hasAttribute(`data-has-overflow-${axis}`)).toBe(overflowing);
        expect(viewport.tabIndex).toBe(overflowing ? 0 : -1);
        expect(viewport.style.getPropertyValue(`--scroll-area-overflow-${axis}-end`))
          .toBe(overflowing ? "200px" : "0px");
        expect(screen.getByLabelText("Resizable pages")).toBe(viewport);
      }
    },
  );

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
