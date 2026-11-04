import { describe, expect, it } from "vitest";
import { layoutSvgText, type LayoutSvgTextOptions } from "@/lib/diligence-canvas/internal/svg/textLayout";
import type { NormalizedTextRun } from "@/lib/diligence-canvas/internal/shared/PowerpointTypes";
const run: NormalizedTextRun = { text: "", fontSize: 10, fontFace: "Arial", color: "000000", bold: false, italic: false, underline: false };
function layout(runs: NormalizedTextRun[], overrides: Partial<LayoutSvgTextOptions> = {}) {
  return layoutSvgText({ runs, fallbackRun: run, fallbackText: "", align: "left", valign: "top", padding: 0,
    width: 100, height: 1000, measure: (text) => text.length * 5, ...overrides });
}
describe("imported paragraph layout", () => {
  it("applies exact line spacing, paragraph gaps and hanging bullet indents", () => {
    const result = layout([{ ...run, text: "•\tAlpha beta gamma delta", breakLine: true,
      paragraph: { marginLeft: 20, indent: -20, lineSpacing: { unit: "points", value: 12 }, spaceBefore: { unit: "points", value: 3 }, spaceAfter: { unit: "points", value: 6 } } },
      { ...run, text: "Next", paragraph: { lineSpacing: { unit: "percent", value: 1.5 } } }]);
    expect(result.lines[0].segments[0].x).toBe(0);
    expect(result.lines[0].segments.find((segment) => segment.text.startsWith("Alpha"))?.x).toBe(20);
    expect(result.lines[1].segments[0].x).toBe(20);
    expect(result.lines[1].baseline - result.lines[0].baseline).toBeCloseTo(16);
    const last = result.lines[result.lines.length - 1];
    expect(last.segments[0].x).toBe(0);
    expect(last.before).toBe(0);
    expect(result.lines[result.lines.length - 2]?.after).toBe(8);
    expect(last.height).toBeCloseTo(15.75);
  });
  it("uses explicit tab stops and preserves leading spaces", () => {
    const result = layout([{ ...run, text: "  A\tB", paragraph: { tabStops: [{ position: 40, align: "left" }] } }]);
    expect(result.lines[0].segments[0].text).toBe("  A");
    expect(result.lines[0].segments[result.lines[0].segments.length - 1]?.x).toBe(40);
  });
  it("positions right and center aligned tab text", () => {
    for (const [align, expected] of [["right", 40], ["center", 45]] as const) {
      const result = layout([{ ...run, text: "A\tBB", paragraph: { tabStops: [{ position: 50, align }] } }]);
      expect(result.lines[0].segments[2].x).toBe(expected);
    }
  });

  it("does not add paragraph spacing on a soft line break and keeps blank paragraphs", () => {
    const result = layout([{ ...run, text: "A\nB", breakLine: true, paragraph: { spaceAfter: { unit: "points", value: 6 } } },
      { ...run, text: "", breakLine: true }, { ...run, text: "C" }]);
    expect(result.lines).toHaveLength(4);
    expect(result.lines[0].after).toBe(0);
    expect(result.lines[1].after).toBe(8);
    expect(result.lines[2].segments).toHaveLength(0);
  });
});
