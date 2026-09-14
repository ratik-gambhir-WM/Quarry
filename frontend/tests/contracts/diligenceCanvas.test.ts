import { describe, expect, it } from "vitest";

import { parseDiligenceCanvasDocument } from "@/contracts/diligenceCanvas";

describe("parseDiligenceCanvasDocument", () => {
  it("validates renderer data while preserving unknown extension fields", () => {
    const input = documentValue();

    const parsed = parseDiligenceCanvasDocument(input);

    expect(parsed).toBe(input);
    expect(parsed.presentation.futureField).toEqual({ kept: true });
    expect(parsed.rootExtension).toBe("kept");
  });

  it("rejects unsupported elements and non-finite geometry", () => {
    const unsupported = documentValue([{ id: "bad", type: "video" }]);
    expect(() => parseDiligenceCanvasDocument(unsupported)).toThrow("supported element type");

    const nonFinite = documentValue([], Number.POSITIVE_INFINITY);
    expect(() => parseDiligenceCanvasDocument(nonFinite)).toThrow("finite number");
  });
});

function documentValue(elements: unknown[] = [], width = 1280) {
  return {
    presentation: {
      preserveElementOrder: true,
      showBranding: false,
      futureField: { kept: true },
      slides: [{
        backgroundColor: "FFFFFF",
        elements,
        height: 720,
        id: "slide-1",
        name: "Slide 1",
        width,
      }],
      title: "Example",
    },
    rootExtension: "kept",
  };
}
