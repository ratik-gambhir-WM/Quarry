import { PlateElement, type PlateElementProps } from "platejs/react";

import { parseTextParagraph, type TextSpacing } from "@/contracts/diligenceText";
import {
  POINTS_TO_SLIDE_UNITS,
  SLIDE_TEXT_LINE_HEIGHT,
} from "../shared/PowerpointConstants";

export function SlideTextParagraph(props: PlateElementProps) {
  const paragraph = parseTextParagraph(props.element.paragraph);
  const spacing = (value: TextSpacing | undefined) =>
    value?.unit === "points"
      ? `${value.value}px`
      : value
        ? `${value.value * SLIDE_TEXT_LINE_HEIGHT}em`
        : undefined;
  const fontSize = getParagraphFontSize(props.element.children);

  return (
    <PlateElement
      {...props}
      style={{
        ...props.style,
        fontSize,
        lineHeight: spacing(paragraph?.lineSpacing) ?? SLIDE_TEXT_LINE_HEIGHT,
        margin: 0,
        padding: 0,
        paddingTop: spacing(paragraph?.spaceBefore) ?? 0,
        paddingBottom: spacing(paragraph?.spaceAfter) ?? 0,
        marginLeft: (paragraph?.marginLeft ?? 0) / POINTS_TO_SLIDE_UNITS,
        marginRight: (paragraph?.marginRight ?? 0) / POINTS_TO_SLIDE_UNITS,
        textIndent: (paragraph?.indent ?? 0) / POINTS_TO_SLIDE_UNITS,
        tabSize: `${(paragraph?.defaultTabSize ?? 48) / POINTS_TO_SLIDE_UNITS}px`,
      }}
    />
  );
}

function getParagraphFontSize(children: unknown) {
  if (!Array.isArray(children)) return undefined;

  const fontSizes = children.flatMap((child) => {
    if (typeof child !== "object" || child === null || !("fontSize" in child)) {
      return [];
    }
    const fontSize = Number.parseFloat(String(child.fontSize));
    return Number.isFinite(fontSize) ? [fontSize] : [];
  });

  return fontSizes.length ? `${Math.max(...fontSizes)}px` : undefined;
}
