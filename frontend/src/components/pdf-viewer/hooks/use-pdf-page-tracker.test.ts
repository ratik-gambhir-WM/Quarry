import { describe, expect, it } from "vitest";
import { pageAtOffset } from "./use-pdf-page-tracker";

describe("pageAtOffset", () => {
  const offsets = [0, 812, 1_624, 2_436];
  const heights = [800, 800, 800, 800];

  it("resolves pages without scanning DOM elements", () => {
    expect(pageAtOffset(offsets, heights, 0)).toBe(1);
    expect(pageAtOffset(offsets, heights, 900)).toBe(2);
    expect(pageAtOffset(offsets, heights, 2_500)).toBe(4);
  });

  it("assigns the inter-page gap to the following page", () => {
    expect(pageAtOffset(offsets, heights, 806)).toBe(2);
  });
});
