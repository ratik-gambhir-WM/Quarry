// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { DiligenceCanvasDocument } from "@/contracts/diligenceCanvas";
import { DiligenceCanvas } from "@/lib/diligence-canvas/DiligenceCanvas";

class TestResizeObserver implements ResizeObserver {
  disconnect() {}
  observe() {}
  unobserve() {}
}

globalThis.ResizeObserver = TestResizeObserver;
HTMLCanvasElement.prototype.getContext = vi.fn(() => null);
afterEach(cleanup);

describe("DiligenceCanvas", () => {
  it("is transport-free and exposes JSON and controlled slide navigation", () => {
    const onSlideIndexChange = vi.fn();
    render(
      <DiligenceCanvas
        onChange={vi.fn()}
        onSlideIndexChange={onSlideIndexChange}
        value={makeDocument()}
      />,
    );

    expect(screen.queryByRole("button", { name: /Import|Export/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Show JSON" }));
    expect(screen.getByRole("complementary", { name: "Presentation JSON" }).textContent)
      .toContain('"title": "Controlled presentation"');

    fireEvent.click(screen.getByRole("button", { name: "Next slide" }));
    expect(onSlideIndexChange).toHaveBeenCalledWith(1);
  });

  it("supports host-owned JSON controls and a canvas-only layout", () => {
    const onJsonOpenChange = vi.fn();
    render(
      <DiligenceCanvas
        isJsonOpen
        jsonPanelId="host-json-panel"
        onChange={vi.fn()}
        onJsonOpenChange={onJsonOpenChange}
        showPresentationHeader={false}
        value={makeDocument()}
      />,
    );

    expect(screen.queryByText("Diligence Canvas")).toBeNull();
    expect(screen.queryByRole("heading", { name: "Controlled presentation" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Show JSON" })).toBeNull();
    expect(screen.getByRole("region", { name: "Controlled presentation" })).not.toBeNull();
    const zoomControls = screen.getByRole("toolbar", { name: "Canvas zoom controls" });
    expect(zoomControls.classList.contains("bottom-4")).toBe(true);
    expect(zoomControls.classList.contains("top-4")).toBe(false);
    const jsonPanel = screen.getByRole("complementary", { name: "Presentation JSON" });
    expect(jsonPanel.id).toBe("host-json-panel");

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onJsonOpenChange).toHaveBeenCalledWith(false);
  });
});

function makeDocument(): DiligenceCanvasDocument {
  const slide = (id: string, name: string) => ({
    backgroundColor: "FFFFFF",
    elements: [],
    height: 720,
    id,
    name,
    width: 1280,
  });
  return {
    presentation: {
      preserveElementOrder: true,
      showBranding: false,
      slides: [slide("slide-1", "Slide 1"), slide("slide-2", "Slide 2")],
      title: "Controlled presentation",
    },
  };
}
