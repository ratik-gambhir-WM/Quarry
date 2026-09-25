import {
  DEFAULT_BACKGROUND_COLOR,
  DEFAULT_CONNECTOR_WIDTH_PT,
  DEFAULT_FONT_FACE,
  DEFAULT_HEIGHT_PX,
  DEFAULT_LINE_WIDTH_PT,
  DEFAULT_OPACITY,
  DEFAULT_SHAPE_FILL_COLOR,
  DEFAULT_SHAPE_FONT_SIZE_PT,
  DEFAULT_STROKE_COLOR,
  DEFAULT_TEXT_COLOR,
  DEFAULT_TEXT_PADDING_PT,
  DEFAULT_WIDTH_PX,
} from './PowerpointConstants'
import type {
  NormalizationOptions,
  JsonObject,
  JsonValue,
  NormalizedElement,
  NormalizedImageElement,
  NormalizedLineElement,
  NormalizedPresentation,
  NormalizedShapeElement,
  NormalizedSlide,
  NormalizedTextElement,
  ValidationIssue,
} from './PowerpointTypes'
import { addConnectorOcclusionRects } from './PowerpointLayering'
import {
  asString,
  cleanHex,
  clamp01,
  coerceBoolean,
  coerceNumber,
  inchesToPx,
  isDefined,
  isRecord,
  normalizeAlign,
  normalizeArrow,
  normalizeDash,
  normalizeElbowDirection,
  normalizeImageFit,
  normalizeLineType,
  normalizeNativeKind,
  normalizeShapeName,
  normalizeValign,
  resolveImageSource,
} from './PowerpointUtils'

export function normalizeNativePresentation(
  input: JsonObject,
  issues: ValidationIssue[],
  options: NormalizationOptions,
): NormalizedPresentation | undefined {
  const presentationNode = isRecord(input.presentation) ? input.presentation : input
  const slidesSource = Array.isArray(presentationNode.slides)
    ? presentationNode.slides
    : isRecord(presentationNode.slide) && Array.isArray(presentationNode.slide.elements)
      ? [presentationNode.slide]
      : Array.isArray(input.slides)
        ? input.slides
        : Array.isArray(input.elements)
          ? [input]
          : undefined

  if (!slidesSource?.length) {
    issues.push({
      level: 'error',
      path: 'slides',
      message:
        'Expected either an extracted PowerPoint slide payload (`shapeTree`) or a slide/deck object with `slides` or `elements`.',
    })
    return undefined
  }

  const width = resolveNativeDimension(
    presentationNode,
    presentationNode.canvas,
    presentationNode.size,
    presentationNode.layout,
    'width',
    DEFAULT_WIDTH_PX,
  )
  const height = resolveNativeDimension(
    presentationNode,
    presentationNode.canvas,
    presentationNode.size,
    presentationNode.layout,
    'height',
    DEFAULT_HEIGHT_PX,
  )

  const mappedSlides: Array<NormalizedSlide | undefined> = slidesSource
    .map((slideSource, index) =>
      normalizeNativeSlide(
        slideSource,
        index,
        width,
        height,
        issues,
        options,
        presentationNode.preserveElementOrder === true,
      ),
    )
  const slides = mappedSlides.filter(isDefined)

  if (!slides.length) {
    issues.push({
      level: 'error',
      path: 'slides',
      message: 'The deck did not contain any valid slides after normalization.',
    })
    return undefined
  }

  return {
    meta: {
      title: asString(presentationNode.title) || 'Generated Presentation',
      width: slides[0].width,
      height: slides[0].height,
      preserveElementOrder: presentationNode.preserveElementOrder === true,
      showBranding: presentationNode.showBranding !== false,
      sourceType: 'native-presentation',
    },
    slides,
  }
}

export function normalizeNativeSlide(
  slideSource: JsonValue | undefined,
  index: number,
  defaultWidth: number,
  defaultHeight: number,
  issues: ValidationIssue[],
  options: NormalizationOptions,
  preserveElementOrder = false,
): NormalizedSlide | undefined {
  if (!isRecord(slideSource)) {
    issues.push({
      level: 'warning',
      path: `slides[${index}]`,
      message: 'Skipped a slide because it was not an object.',
    })
    return undefined
  }

  const width = resolveNativeDimension(
    slideSource,
    slideSource.canvas,
    slideSource.size,
    slideSource.layout,
    'width',
    defaultWidth,
  )
  const height = resolveNativeDimension(
    slideSource,
    slideSource.canvas,
    slideSource.size,
    slideSource.layout,
    'height',
    defaultHeight,
  )
  const elementsSource = Array.isArray(slideSource.elements) ? slideSource.elements : []
  const mappedElements: Array<NormalizedElement | undefined> = elementsSource
    .map((item, elementIndex) =>
      normalizeNativeElement(item, width, height, issues, `slides[${index}].elements[${elementIndex}]`, options),
    )
  const normalizedElements = mappedElements.filter(isDefined)
  const elements = preserveElementOrder
    ? normalizedElements
    : addConnectorOcclusionRects(normalizedElements, { height, width })

  return {
    id: asString(slideSource.id) || `slide-${index + 1}`,
    name: asString(slideSource.name) || asString(slideSource.title) || `Slide ${index + 1}`,
    width,
    height,
    backgroundColor: cleanHex(asString(slideSource.backgroundColor), DEFAULT_BACKGROUND_COLOR),
    preserveElementOrder,
    elements,
  }
}

function normalizeNativeElement(
  input: JsonValue | undefined,
  width: number,
  height: number,
  issues: ValidationIssue[],
  pathLabel: string,
  options: NormalizationOptions,
): NormalizedElement | undefined {
  if (!isRecord(input)) {
    issues.push({
      level: 'warning',
      path: pathLabel,
      message: 'Skipped a non-object element.',
    })
    return undefined
  }

  const rawKind =
    asString(input.kind) || asString(input.type) || asString(input.elementType) || asString(input.shape)
  const kind = normalizeNativeKind(rawKind)

  if (!kind) {
    issues.push({
      level: 'warning',
      path: pathLabel,
      message: `Unsupported element type "${rawKind ?? 'unknown'}".`,
    })
    return undefined
  }

  if (kind === 'line') {
    const x1 = resolvePosition(input.x1 ?? input.x ?? input.left, width)
    const y1 = resolvePosition(input.y1 ?? input.y ?? input.top, height)
    const x2 =
      input.x2 !== undefined
        ? resolvePosition(input.x2, width)
        : x1 + resolvePosition(input.w ?? input.width ?? 0, width)
    const y2 =
      input.y2 !== undefined
        ? resolvePosition(input.y2, height)
        : y1 + resolvePosition(input.h ?? input.height ?? 0, height)
    const lineType = normalizeLineType(asString(input.lineType), x1, y1, x2, y2)

    const element: NormalizedLineElement = {
      kind: 'line',
      id: asString(input.id) || pathLabel,
      sourcePath: pathLabel,
      opacity: clamp01(coerceNumber(input.opacity, DEFAULT_OPACITY)),
      rotate: coerceNumber(input.rotate, 0),
      flipH: coerceBoolean(input.flipH) || undefined,
      flipV: coerceBoolean(input.flipV) || undefined,
      valign: 'middle',
      lineType,
      elbowDirection: lineType === 'elbow'
        ? normalizeElbowDirection(asString(input.elbowDirection), x1, y1, x2, y2)
        : undefined,
      x1,
      y1,
      x2,
      y2,
      stroke: cleanHex(asString(input.stroke) || asString(input.color), DEFAULT_STROKE_COLOR),
      strokeOpacity: clamp01(coerceNumber(input.strokeOpacity, DEFAULT_OPACITY)),
      strokeWidth: coerceNumber(input.strokeWidth, DEFAULT_CONNECTOR_WIDTH_PT),
      dash: normalizeDash(asString(input.dash)),
      beginArrow: normalizeArrow(asString(input.beginArrow) || asString(input.startArrow)),
      endArrow: normalizeArrow(asString(input.endArrow) || asString(input.arrow)),
      occlusionRects: [],
    }
    return element
  }

  if (kind === 'image') {
    const src = resolveImageSource(asString(input.src) || asString(input.path) || asString(input.data), options)
    if (!src) {
      issues.push({
        level: 'warning',
        path: `${pathLabel}.src`,
        message: 'Skipped an image element because it did not include a usable image source.',
      })
      return undefined
    }

    const element: NormalizedImageElement = {
      kind: 'image',
      id: asString(input.id) || pathLabel,
      sourcePath: pathLabel,
      opacity: clamp01(coerceNumber(input.opacity, DEFAULT_OPACITY)),
      rotate: coerceNumber(input.rotate, 0),
      flipH: coerceBoolean(input.flipH) || undefined,
      flipV: coerceBoolean(input.flipV) || undefined,
      valign: 'middle',
      x: resolvePosition(input.x ?? input.left, width),
      y: resolvePosition(input.y ?? input.top, height),
      w: resolvePosition(input.w ?? input.width, width),
      h: resolvePosition(input.h ?? input.height, height),
      src,
      fit: normalizeImageFit(asString(input.fit)),
      crop: normalizeImageCrop(input.crop),
      borderRadius: coerceNumber(input.borderRadius, 0),
      altText: asString(input.altText) || '',
    }
    return element
  }

  const x = resolvePosition(input.x ?? input.left, width)
  const y = resolvePosition(input.y ?? input.top, height)
  const w = resolvePosition(input.w ?? input.width, width)
  const h = resolvePosition(input.h ?? input.height, height)
  const padding = coerceNumber(input.padding, DEFAULT_TEXT_PADDING_PT)
  const align = normalizeAlign(asString(input.align))
  const valign = normalizeValign(asString(input.valign))
  const fontSize = coerceNumber(input.fontSize, DEFAULT_SHAPE_FONT_SIZE_PT)
  const fontFace = asString(input.fontFace) || DEFAULT_FONT_FACE
  const textColor = cleanHex(asString(input.color) || asString(input.textColor), DEFAULT_TEXT_COLOR)
  const textContent = asString(input.text) || asString(input.label) || ''
  const nativeRuns = normalizeNativeTextRuns(input.runs, fontFace, fontSize, textColor)
  const runs =
    nativeRuns.length > 0
      ? nativeRuns
      : textContent
        ? [
            {
              text: textContent,
              bold: coerceBoolean(input.bold),
              italic: coerceBoolean(input.italic),
              underline: false,
              fontFace,
              fontSize,
              color: textColor,
            },
          ]
        : []

  if (kind === 'text') {
    const element: NormalizedTextElement = {
      kind: 'text',
      id: asString(input.id) || pathLabel,
      sourcePath: pathLabel,
      opacity: clamp01(coerceNumber(input.opacity, DEFAULT_OPACITY)),
      rotate: coerceNumber(input.rotate, 0),
      flipH: coerceBoolean(input.flipH) || undefined,
      flipV: coerceBoolean(input.flipV) || undefined,
      valign,
      x,
      y,
      w,
      h,
      text: textContent,
      fill: cleanHex(asString(input.fill), DEFAULT_BACKGROUND_COLOR),
      fillOpacity: clamp01(coerceNumber(input.fillOpacity, DEFAULT_OPACITY)),
      stroke: cleanHex(asString(input.stroke), DEFAULT_BACKGROUND_COLOR),
      strokeOpacity: clamp01(coerceNumber(input.strokeOpacity, DEFAULT_OPACITY)),
      strokeWidth: coerceNumber(input.strokeWidth, 0),
      borderRadius: coerceNumber(input.borderRadius, 0),
      padding,
      align,
      color: textColor,
      fontSize,
      fontFace,
      bold: coerceBoolean(input.bold),
      italic: coerceBoolean(input.italic),
      runs,
    }
    return element
  }

  const element: NormalizedShapeElement = {
    kind: 'shape',
    id: asString(input.id) || pathLabel,
    sourcePath: pathLabel,
    opacity: clamp01(coerceNumber(input.opacity, DEFAULT_OPACITY)),
    rotate: coerceNumber(input.rotate, 0),
    flipH: coerceBoolean(input.flipH) || undefined,
    flipV: coerceBoolean(input.flipV) || undefined,
    valign,
    x,
    y,
    w,
    h,
    shape: normalizeShapeName(asString(input.shape) || rawKind),
    label: textContent,
    fill: cleanHex(asString(input.fill), DEFAULT_SHAPE_FILL_COLOR),
    fillOpacity: clamp01(coerceNumber(input.fillOpacity, DEFAULT_OPACITY)),
    stroke: cleanHex(asString(input.stroke), DEFAULT_STROKE_COLOR),
    strokeOpacity: clamp01(coerceNumber(input.strokeOpacity, DEFAULT_OPACITY)),
    strokeWidth: coerceNumber(input.strokeWidth, DEFAULT_LINE_WIDTH_PT),
    borderRadius: coerceNumber(input.borderRadius, 0),
    padding,
    align,
    textColor,
    fontSize,
    fontFace,
    bold: coerceBoolean(input.bold),
    textRuns: runs,
  }
  return element
}

function normalizeImageCrop(input: JsonValue | undefined): NormalizedImageElement['crop'] {
  if (!isRecord(input)) {
    return undefined
  }

  const crop = {
    top: clamp01(coerceNumber(input.top, 0)),
    right: clamp01(coerceNumber(input.right, 0)),
    bottom: clamp01(coerceNumber(input.bottom, 0)),
    left: clamp01(coerceNumber(input.left, 0)),
  }

  return crop.top || crop.right || crop.bottom || crop.left ? crop : undefined
}

function normalizeNativeTextRuns(
  input: JsonValue | undefined,
  fallbackFontFace: string,
  fallbackFontSize: number,
  fallbackColor: string,
) {
  if (!Array.isArray(input)) {
    return []
  }

  return input
    .filter((run): run is JsonObject => isRecord(run))
    .map((run) => ({
      text: asString(run.text) || '',
      bold: coerceBoolean(run.bold),
      italic: coerceBoolean(run.italic),
      underline: coerceBoolean(run.underline),
      color: cleanHex(asString(run.color), fallbackColor),
      fontFace: asString(run.fontFace) || fallbackFontFace,
      fontSize: coerceNumber(run.fontSize, fallbackFontSize),
      breakLine: coerceBoolean(run.breakLine) || undefined,
    }))
}

function resolveNativeDimension(
  source: JsonValue | undefined,
  canvas: JsonValue | undefined,
  size: JsonValue | undefined,
  layout: JsonValue | undefined,
  axis: 'width' | 'height',
  fallback: number,
) {
  const sourceNode = isRecord(source) ? source : undefined
  const canvasNode = isRecord(canvas) ? canvas : undefined
  const sizeNode = isRecord(size) ? size : undefined
  const layoutNode = isRecord(layout) ? layout : undefined
  const key = axis === 'width' ? 'width' : 'height'
  const inchesKey = axis === 'width' ? 'widthInches' : 'heightInches'

  const pixelValue =
    sourceNode?.[key] ??
    canvasNode?.[key] ??
    sizeNode?.[key] ??
    layoutNode?.[key] ??
    layoutNode?.[axis === 'width' ? 'w' : 'h']

  if (typeof pixelValue === 'number') {
    return pixelValue
  }

  const inchesValue =
    sourceNode?.[inchesKey] ??
    canvasNode?.[inchesKey] ??
    sizeNode?.[inchesKey] ??
    layoutNode?.[inchesKey]

  if (typeof inchesValue === 'number') {
    return inchesToPx(inchesValue)
  }

  return fallback
}

function resolvePosition(value: JsonValue | undefined, canvasSize: number) {
  if (typeof value === 'number') {
    return value
  }

  if (typeof value === 'string') {
    const trimmed = value.trim()
    if (trimmed.endsWith('%')) {
      const percent = Number(trimmed.slice(0, -1))
      return Number.isFinite(percent) ? (canvasSize * percent) / 100 : 0
    }
    if (trimmed.endsWith('in')) {
      const inches = Number(trimmed.slice(0, -2))
      return Number.isFinite(inches) ? inchesToPx(inches) : 0
    }
    const numeric = Number(trimmed)
    return Number.isFinite(numeric) ? numeric : 0
  }

  return 0
}
