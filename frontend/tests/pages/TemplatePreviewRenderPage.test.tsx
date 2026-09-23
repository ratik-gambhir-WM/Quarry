// @vitest-environment happy-dom

import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import type { JsonValue } from "@/contracts/diligenceCanvas";
import { TemplatePreviewRenderPage } from "@/pages/TemplatePreviewRenderPage";

const input: JsonValue = {
  presentation: {
    preserveElementOrder: true,
    showBranding: false,
    slides: [{
      backgroundColor: "FFFFFF",
      elements: [{
        color: "070154",
        fill: "transparent",
        fontFace: "Arial",
        fontSize: 20,
        h: 60,
        id: "title",
        text: "Preview title",
        type: "text",
        w: 400,
        x: 40,
        y: 40,
      }],
      height: 720,
      id: "slide-1",
      name: "Preview slide",
      width: 1280,
    }],
    title: "Preview test",
  },
};

afterEach(() => {
  cleanup();
  delete window.__TTS_MERMAID_TEMPLATE_PREVIEW_INPUT__;
  window.history.replaceState({}, "", "/");
});

describe("TemplatePreviewRenderPage", () => {
  it("reports an error when the server did not inject preview input", () => {
    const { container } = render(<TemplatePreviewRenderPage />);

    expect(container.querySelector('[data-preview-state="error"]')?.textContent)
      .toBe("No template preview input was provided.");
    expect(container.querySelector("[data-template-preview-surface]")).toBeNull();
  });

  it("renders only the injected slide at the requested maximum dimension", () => {
    window.__TTS_MERMAID_TEMPLATE_PREVIEW_INPUT__ = input;
    window.history.replaceState({}, "", "/?maxDimension=1600");

    const { container } = render(<TemplatePreviewRenderPage />);

    expect(container.querySelector('[data-preview-state="ready"]')).not.toBeNull();
    const surface = container.querySelector("[data-template-preview-surface]");
    expect(surface?.getAttribute("width")).toBe("1600");
    expect(surface?.getAttribute("height")).toBe("900");
    expect(surface?.getAttribute("viewBox")).toBe("0 0 1280 720");
    expect(container.textContent).toContain("Preview title");
  });

  it.each([
    ["invalid", "1600"],
    ["200", "320"],
    ["999999", "4096"],
  ])("bounds maxDimension=%s to a safe render width", (value, expectedWidth) => {
    window.__TTS_MERMAID_TEMPLATE_PREVIEW_INPUT__ = input;
    window.history.replaceState({}, "", `/?maxDimension=${value}`);

    const { container } = render(<TemplatePreviewRenderPage />);

    expect(container.querySelector("[data-template-preview-surface]")?.getAttribute("width"))
      .toBe(expectedWidth);
  });

  it("does not expose a capture surface for invalid slide JSON", () => {
    window.__TTS_MERMAID_TEMPLATE_PREVIEW_INPUT__ = {};

    const { container } = render(<TemplatePreviewRenderPage />);

    expect(container.querySelector('[data-preview-state="error"]')?.textContent)
      .toBe("Unable to render this slide template.");
    expect(container.querySelector("[data-template-preview-surface]")).toBeNull();
  });
});
