/** Paragraph lengths use slide pixels; point spacing uses typographic points. */
export type TextSpacing = { unit: "points" | "percent"; value: number };
export type TextParagraph = {
  marginLeft?: number;
  marginRight?: number;
  indent?: number;
  defaultTabSize?: number;
  tabStops?: { position: number; align: "left" | "center" | "right" }[];
  lineSpacing?: TextSpacing;
  spaceBefore?: TextSpacing;
  spaceAfter?: TextSpacing;
  align?: "left" | "center" | "right";
};

export function parseTextParagraph(value: unknown): TextParagraph | undefined {
  if (value === undefined) return undefined;
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError("Text paragraph must be an object.");
  }
  const source = value as Record<string, unknown>;
  const result: TextParagraph = {};
  const lengths = ["marginLeft", "marginRight", "indent", "defaultTabSize"] as const;
  for (const key of lengths) {
    const number = source[key];
    if (number === undefined) continue;
    if (!isBoundedNumber(number) || (key === "defaultTabSize" && number <= 0)) {
      throw new TypeError(`Invalid paragraph ${key}.`);
    }
    result[key] = number;
  }
  for (const key of ["lineSpacing", "spaceBefore", "spaceAfter"] as const) {
    const spacing = source[key];
    if (spacing === undefined) continue;
    if (
      !spacing ||
      typeof spacing !== "object" ||
      !("unit" in spacing) ||
      !("value" in spacing) ||
      (spacing.unit !== "points" && spacing.unit !== "percent") ||
      !isBoundedNumber(spacing.value) ||
      spacing.value < 0
    ) {
      throw new TypeError(`Invalid paragraph ${key}.`);
    }
    result[key] = { unit: spacing.unit, value: spacing.value };
  }
  if (source.align !== undefined) {
    if (!isAlignment(source.align)) {
      throw new TypeError("Invalid paragraph alignment.");
    }
    result.align = source.align;
  }
  if (source.tabStops !== undefined) {
    if (!Array.isArray(source.tabStops) || source.tabStops.length > 1000) {
      throw new TypeError("Invalid paragraph tab stops.");
    }
    result.tabStops = source.tabStops.map((tab: unknown) => {
      if (
        !tab ||
        typeof tab !== "object" ||
        !("position" in tab) ||
        !isBoundedNumber(tab.position) ||
        !("align" in tab) ||
        !isAlignment(tab.align)
      ) {
        throw new TypeError("Invalid paragraph tab stop.");
      }
      return { position: tab.position, align: tab.align };
    });
  }
  return result;
}

function isBoundedNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 1_000_000;
}

function isAlignment(value: unknown): value is "left" | "center" | "right" {
  return value === "left" || value === "center" || value === "right";
}
