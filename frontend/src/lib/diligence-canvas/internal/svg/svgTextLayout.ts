import {
  DEFAULT_FONT_FACE,
  POINTS_TO_SLIDE_UNITS,
} from '../shared/PowerpointConstants'
import type {
  NormalizedShapeElement,
  NormalizedTextElement,
  NormalizedTextRun,
} from '../shared/PowerpointTypes'
import type { SlideElementRef } from '../canvas/model'
import { createCanvasTextMeasurer, layoutSvgText } from './textLayout'

const measureText = createCanvasTextMeasurer()

export function getSvgTextContent(
  element: NormalizedShapeElement | NormalizedTextElement,
  fixedFontScale?: number,
) {
  const text = element.kind === 'shape' ? element.label : element.text
  const sourceRuns = element.kind === 'shape' ? element.textRuns : element.runs
  const runs = sourceRuns.map((run) => ({
    ...run,
    fontFace: run.fontFace || DEFAULT_FONT_FACE,
    fontSize: run.fontSize * POINTS_TO_SLIDE_UNITS,
  }))
  const fallbackRun = createFallbackRun(element, text)
  const layout = layoutSvgText({
    align: element.align,
    fallbackRun,
    fallbackText: text,
    fixedFontScale,
    height: element.h,
    measure: measureText,
    padding: element.padding,
    runs,
    valign: element.valign,
    width: element.w,
  })

  return { fallbackRun, layout, runs, text }
}

export function getConsistentSvgTextFontScales(elementRefs: SlideElementRef[]) {
  const groups = new Map<
    string,
    Array<{ fontScale: number; key: string }>
  >()

  elementRefs.forEach((elementRef) => {
    const element = elementRef.element
    if (element.kind !== 'shape' && element.kind !== 'text') {
      return
    }

    const textContent = getSvgTextContent(element)
    if (!textContent.text || textContent.runs.length < 2) {
      return
    }

    const styleProfile = getTextStyleProfile(textContent.runs)
    const groupKey = [
      element.kind,
      element.x.toFixed(2),
      element.w.toFixed(2),
      element.padding.toFixed(2),
      element.align,
      element.valign,
      styleProfile,
    ].join(':')
    const group = groups.get(groupKey) ?? []
    group.push({
      fontScale: textContent.layout.fontScale,
      key: elementRef.key,
    })
    groups.set(groupKey, group)
  })

  const fontScales = new Map<string, number>()
  groups.forEach((group) => {
    if (group.length < 2) {
      return
    }

    const sharedScale = Math.min(...group.map(({ fontScale }) => fontScale))
    group.forEach(({ key }) => fontScales.set(key, sharedScale))
  })
  return fontScales
}

export function getSvgTextScaleTemplateKey(elementRefs: SlideElementRef[]) {
  return elementRefs
    .flatMap((elementRef) => {
      const element = elementRef.element
      if (element.kind !== 'shape' && element.kind !== 'text') {
        return []
      }

      const textContent = getSvgTextContent(element)
      const styleRuns =
        textContent.runs.length > 0
          ? textContent.runs
          : [textContent.fallbackRun]
      return [
        [
          elementRef.key,
          element.kind,
          element.x.toFixed(2),
          element.y.toFixed(2),
          element.w.toFixed(2),
          element.h.toFixed(2),
          element.padding.toFixed(2),
          element.align,
          element.valign,
          getTextStyleProfile(styleRuns),
        ].join(':'),
      ]
    })
    .join('||')
}

function getTextStyleProfile(runs: NormalizedTextRun[]) {
  return Array.from(
    new Set(
      runs.map((run) =>
        [
          run.fontFace,
          run.fontSize,
          run.bold,
          run.italic,
          run.underline,
          run.color,
        ].join('|'),
      ),
    ),
  ).join(';')
}

function createFallbackRun(
  element: NormalizedShapeElement | NormalizedTextElement,
  text: string,
): NormalizedTextRun {
  return {
    bold: element.bold,
    color: element.kind === 'shape' ? element.textColor : element.color,
    fontFace: element.fontFace || DEFAULT_FONT_FACE,
    fontSize: element.fontSize * POINTS_TO_SLIDE_UNITS,
    italic: element.kind === 'text' ? element.italic : false,
    text,
    underline: false,
  }
}
