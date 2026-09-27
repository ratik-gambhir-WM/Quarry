import type {
  PowerPointCanvasElement,
  PowerPointCanvasImageElement,
  PowerPointCanvasJson,
  PowerPointCanvasLineElement,
  PowerPointCanvasShapeElement,
  PowerPointCanvasTextElement,
  PowerPointCanvasTextRun,
} from '../lib/import/PowerpointImportTypes'
import { normalizeElbowDirection, normalizeLineType } from '../lib/shared/PowerpointUtils'
import { ApiError } from '../errors'

const DEFAULT_WIDTH = 1280
const DEFAULT_HEIGHT = 720
const DEFAULT_FONT = 'Aptos'
const MAX_RUST_I64 = 9_223_372_036_854_776_000

type JsonRecord = Record<string, unknown>
type Slide = PowerPointCanvasJson['presentation']['slides'][number]

export function normalizePresentation(input: unknown): {
  templateJson: PowerPointCanvasJson
  warnings: string[]
} {
  const warnings: string[] = []
  if (Array.isArray(input)) {
    return normalizeParts({}, input, warnings)
  }
  if (!isRecord(input)) {
    throw invalidPresentation()
  }

  const presentationNode = isRecord(input.presentation) ? input.presentation : input
  const slidesSource = findSlides(input, presentationNode)
  if (!slidesSource) {
    throw invalidPresentation()
  }
  return normalizeParts(presentationNode, slidesSource, warnings)
}

function findSlides(root: JsonRecord, presentation: JsonRecord): unknown[] | undefined {
  if (Array.isArray(presentation.slides)) {
    return presentation.slides
  }
  if (Array.isArray(root.slides)) {
    return root.slides
  }
  if (isRecord(presentation.slide) && 'elements' in presentation.slide) {
    return [presentation.slide]
  }
  if ('elements' in root) {
    return [root]
  }
  return undefined
}

function normalizeParts(
  presentationNode: JsonRecord,
  slidesSource: unknown[],
  warnings: string[],
) {
  if (slidesSource.length === 0) {
    throw invalidPresentation()
  }

  const width = dimension(presentationNode, 'width', 'w', DEFAULT_WIDTH)
  const height = dimension(presentationNode, 'height', 'h', DEFAULT_HEIGHT)
  const slides = slidesSource
    .map((value, index) => normalizeSlide(value, index, width, height, warnings))
    .filter((slide): slide is Slide => slide !== undefined)
  if (slides.length === 0) {
    throw invalidPresentation()
  }

  return {
    templateJson: {
      presentation: {
        title: stringValue(presentationNode.title) ?? 'Generated Presentation',
        preserveElementOrder: booleanValue(presentationNode.preserveElementOrder, false),
        showBranding: booleanValue(presentationNode.showBranding, true),
        slides,
      },
    },
    warnings,
  }
}

function normalizeSlide(
  value: unknown,
  index: number,
  defaultWidth: number,
  defaultHeight: number,
  warnings: string[],
): Slide | undefined {
  if (!isRecord(value)) {
    warnings.push(`slides[${index}]: Skipped a non-object slide entry.`)
    return undefined
  }
  const width = dimension(value, 'width', 'w', defaultWidth)
  const height = dimension(value, 'height', 'h', defaultHeight)
  const elementsSource = Array.isArray(value.elements) ? value.elements : []
  const elements = elementsSource
    .map((element, elementIndex) => normalizeElement(
      element,
      width,
      height,
      `slides[${index}].elements[${elementIndex}]`,
      warnings,
    ))
    .filter((element): element is PowerPointCanvasElement => element !== undefined)

  return {
    id: stringValue(value.id) ?? `slide-${index + 1}`,
    name: stringValue(value.name) ?? stringValue(value.title) ?? `Slide ${index + 1}`,
    width,
    height,
    backgroundColor: color(value.backgroundColor, 'FFFFFF'),
    elements,
  }
}

function normalizeElement(
  value: unknown,
  width: number,
  height: number,
  path: string,
  warnings: string[],
): PowerPointCanvasElement | undefined {
  if (!isRecord(value)) {
    warnings.push(`${path}: Skipped a non-object element.`)
    return undefined
  }

  const rawKind = firstString(value, ['kind', 'type', 'elementType', 'shape']) ?? ''
  const kind = rawKind.toLowerCase()
  const id = stringValue(value.id) ?? path
  const opacity = clamp01(numberValue(value.opacity, 1))
  const rotate = numberValue(value.rotate, 0)

  if (['line', 'connector', 'arrow', 'elbow'].includes(kind)) {
    return normalizeLine(value, width, height, id, opacity, rotate)
  }
  if (['image', 'img', 'picture', 'photo'].includes(kind)) {
    return normalizeImage(value, width, height, id, opacity, rotate, path, warnings)
  }

  const isText = ['text', 'textbox', 'text-box'].includes(kind)
  const isKnownShape = [
    'shape',
    'rect',
    'rectangle',
    'roundrect',
    'roundedrectangle',
    'ellipse',
    'circle',
    'triangle',
    'diamond',
    'hexagon',
    'chevron',
    'arc',
  ].includes(kind)
  if (!isText && !isKnownShape) {
    warnings.push(`${path}: Unsupported element type "${rawKind || 'unknown'}".`)
    return undefined
  }

  return normalizeTextOrShape(value, width, height, id, opacity, rotate, rawKind, isText)
}

function normalizeLine(
  source: JsonRecord,
  width: number,
  height: number,
  id: string,
  opacity: number,
  rotate: number,
): PowerPointCanvasLineElement {
  const x1 = position(first(source, ['x1', 'x', 'left']), width)
  const y1 = position(first(source, ['y1', 'y', 'top']), height)
  const x2 = source.x2 === undefined
    ? x1 + position(first(source, ['w', 'width']), width)
    : position(source.x2, width)
  const y2 = source.y2 === undefined
    ? y1 + position(first(source, ['h', 'height']), height)
    : position(source.y2, height)
  const dash = dashStyle(stringValue(source.dash))
  const beginArrow = arrowType(stringValue(source.beginArrow) ?? stringValue(source.startArrow))
  const endArrow = arrowType(stringValue(source.endArrow) ?? stringValue(source.arrow))
  const lineType = normalizeLineType(stringValue(source.lineType), x1, y1, x2, y2)

  return {
    id,
    type: 'line',
    lineType,
    elbowDirection: lineType === 'elbow'
      ? normalizeElbowDirection(stringValue(source.elbowDirection), x1, y1, x2, y2)
      : undefined,
    x1,
    y1,
    x2,
    y2,
    stroke: color(first(source, ['stroke', 'color']), '334155'),
    strokeWidth: numberValue(source.strokeWidth, 1.5),
    dash,
    beginArrow,
    endArrow,
    opacity,
    strokeOpacity: clamp01(numberValue(source.strokeOpacity, 1)),
    rotate,
  }
}

function normalizeImage(
  source: JsonRecord,
  width: number,
  height: number,
  id: string,
  opacity: number,
  rotate: number,
  path: string,
  warnings: string[],
): PowerPointCanvasImageElement | undefined {
  const sourceValue = first(source, ['src', 'path', 'data'])
  if (typeof sourceValue !== 'string') {
    warnings.push(
      `${path}.src: Skipped an image element because it did not include a usable image source.`,
    )
    return undefined
  }

  return {
    id,
    type: 'image',
    x: position(first(source, ['x', 'left']), width),
    y: position(first(source, ['y', 'top']), height),
    w: position(first(source, ['w', 'width']), width),
    h: position(first(source, ['h', 'height']), height),
    src: sourceValue,
    fit: imageFit(stringValue(source.fit)),
    crop: imageCrop(source.crop),
    altText: stringValue(source.altText) ?? '',
    opacity,
    rotate,
    flipH: booleanValue(source.flipH, false),
    flipV: booleanValue(source.flipV, false),
    borderRadius: numberValue(source.borderRadius, 0),
  }
}

function normalizeTextOrShape(
  source: JsonRecord,
  width: number,
  height: number,
  id: string,
  opacity: number,
  rotate: number,
  rawKind: string,
  isText: boolean,
): PowerPointCanvasTextElement | PowerPointCanvasShapeElement {
  const text = stringValue(first(source, ['text', 'label'])) ?? ''
  const fontFace = stringValue(source.fontFace) ?? DEFAULT_FONT
  const fontSize = numberValue(source.fontSize, 18)
  const textColor = color(first(source, ['color', 'textColor']), '111827')
  const bold = booleanValue(source.bold, false)
  const italic = booleanValue(source.italic, false)
  const runs = textRuns(source.runs, text, fontFace, fontSize, textColor, bold, italic)
  const common = {
    id,
    x: position(first(source, ['x', 'left']), width),
    y: position(first(source, ['y', 'top']), height),
    w: position(first(source, ['w', 'width']), width),
    h: position(first(source, ['h', 'height']), height),
    text,
    strokeWidth: numberValue(source.strokeWidth, isText ? 0 : 1),
    textColor,
    fontSize,
    fontFace,
    padding: numberValue(source.padding, 8),
    align: horizontalAlign(stringValue(source.align)),
    valign: verticalAlign(stringValue(source.valign)),
    runs,
    opacity,
    rotate,
    bold,
    flipH: booleanValue(source.flipH, false),
    flipV: booleanValue(source.flipV, false),
    borderRadius: numberValue(source.borderRadius, 0),
    fillOpacity: clamp01(numberValue(source.fillOpacity, 1)),
    strokeOpacity: clamp01(numberValue(source.strokeOpacity, 1)),
  } as const

  if (isText) {
    return {
      ...common,
      type: 'text',
      fill: color(source.fill, 'FFFFFF'),
      stroke: color(source.stroke, 'FFFFFF'),
      italic,
    }
  }
  return {
    ...common,
    type: 'shape',
    shape: shapeName(stringValue(source.shape) ?? rawKind),
    fill: color(source.fill, 'E5EEF8'),
    stroke: color(source.stroke, '334155'),
  }
}

function textRuns(
  value: unknown,
  fallbackText: string,
  fallbackFont: string,
  fallbackSize: number,
  fallbackColor: string,
  fallbackBold: boolean,
  fallbackItalic: boolean,
): PowerPointCanvasTextRun[] {
  const runs = Array.isArray(value)
    ? value.filter(isRecord).map((run) => ({
        text: stringValue(run.text) ?? '',
        color: color(run.color, fallbackColor),
        fontFace: stringValue(run.fontFace) ?? fallbackFont,
        fontSize: numberValue(run.fontSize, fallbackSize),
        bold: booleanValue(run.bold, false),
        italic: booleanValue(run.italic, false),
        underline: booleanValue(run.underline, false),
        breakLine: booleanValue(run.breakLine, false),
      }))
    : []
  if (runs.length === 0 && fallbackText.length > 0) {
    runs.push({
      text: fallbackText,
      color: fallbackColor,
      fontFace: fallbackFont,
      fontSize: fallbackSize,
      bold: fallbackBold,
      italic: fallbackItalic,
      underline: false,
      breakLine: false,
    })
  }
  return runs
}

function dimension(
  source: JsonRecord,
  key: 'height' | 'width',
  short: 'h' | 'w',
  fallback: number,
) {
  if (typeof source[key] === 'number') {
    return source[key]
  }
  for (const container of ['canvas', 'size', 'layout']) {
    const nested = source[container]
    if (!isRecord(nested)) {
      continue
    }
    const value = key in nested ? nested[key] : nested[short]
    if (typeof value === 'number') {
      return value
    }
    const inches = nested[key === 'width' ? 'widthInches' : 'heightInches']
    if (typeof inches === 'number') {
      return inches * 96
    }
  }
  return fallback
}

function first(source: JsonRecord, keys: string[]) {
  for (const key of keys) {
    if (key in source) {
      return source[key]
    }
  }
  return undefined
}

function firstString(source: JsonRecord, keys: string[]) {
  for (const key of keys) {
    const value = stringValue(source[key])
    if (value !== undefined) {
      return value
    }
  }
  return undefined
}

function stringValue(value: unknown) {
  return typeof value === 'string' && value.length > 0 ? value : undefined
}

function numberValue(value: unknown, fallback: number) {
  if (typeof value === 'number') {
    return Number.isFinite(value) ? value : fallback
  }
  if (typeof value === 'string') {
    return parseFiniteFloat(value) ?? fallback
  }
  return fallback
}

function position(value: unknown, canvasSize: number) {
  if (typeof value === 'number') {
    return value
  }
  if (typeof value !== 'string') {
    return 0
  }
  const trimmed = value.trim()
  if (trimmed.endsWith('%')) {
    const percent = parseFiniteFloat(trimmed.slice(0, -1))
    return percent === undefined ? 0 : (canvasSize * percent) / 100
  }
  if (trimmed.endsWith('in')) {
    const inches = parseFiniteFloat(trimmed.slice(0, -2))
    return inches === undefined ? 0 : inches * 96
  }
  return parseFiniteFloat(trimmed) ?? 0
}

function booleanValue(value: unknown, fallback: boolean) {
  if (typeof value === 'boolean') {
    return value
  }
  if (typeof value === 'number') {
    return Number.isInteger(value) && Math.abs(value) <= MAX_RUST_I64 && value !== 0
  }
  if (typeof value === 'string') {
    return ['true', '1', 'yes'].includes(value.toLowerCase())
  }
  return fallback
}

function color(value: unknown, fallback: string) {
  if (typeof value !== 'string') {
    return fallback
  }
  if (value.toLowerCase() === 'transparent') {
    return 'transparent'
  }
  const clean = value.trim().replace(/^#+/u, '')
  return (clean.length === 6 || clean.length === 8) && /^[0-9a-f]+$/iu.test(clean)
    ? clean.slice(0, 6).toUpperCase()
    : fallback
}

function horizontalAlign(value: string | undefined): 'center' | 'left' | 'right' {
  const normalized = value?.toLowerCase()
  if (normalized === 'center' || normalized === 'ctr') {
    return 'center'
  }
  return normalized === 'right' || normalized === 'r' ? 'right' : 'left'
}

function verticalAlign(value: string | undefined): 'bottom' | 'middle' | 'top' {
  const normalized = value?.toLowerCase()
  if (normalized === 'top' || normalized === 't') {
    return 'top'
  }
  return normalized === 'bottom' || normalized === 'b' ? 'bottom' : 'middle'
}

function dashStyle(value: string | undefined): 'dash' | 'dot' | 'solid' {
  const normalized = value?.toLowerCase()
  if (normalized === 'dash' || normalized === 'dashed') {
    return 'dash'
  }
  return normalized === 'dot' || normalized === 'dotted' ? 'dot' : 'solid'
}

function arrowType(value: string | undefined): PowerPointCanvasLineElement['endArrow'] {
  const normalized = value?.toLowerCase()
  return normalized === 'triangle'
    || normalized === 'arrow'
    || normalized === 'diamond'
    || normalized === 'oval'
    || normalized === 'stealth'
    ? normalized
    : undefined
}

function imageFit(value: string | undefined): PowerPointCanvasImageElement['fit'] {
  const normalized = value?.toLowerCase()
  return normalized === 'cover' || normalized === 'stretch' ? normalized : 'contain'
}

function imageCrop(value: unknown): PowerPointCanvasImageElement['crop'] {
  if (!isRecord(value)) {
    return undefined
  }
  const crop = {
    top: clamp01(numberValue(value.top, 0)),
    right: clamp01(numberValue(value.right, 0)),
    bottom: clamp01(numberValue(value.bottom, 0)),
    left: clamp01(numberValue(value.left, 0)),
  }
  return crop.top + crop.right + crop.bottom + crop.left > 0 ? crop : undefined
}

function shapeName(value: string) {
  const normalized = value.toLowerCase()
  if (normalized === 'rectangle' || normalized === 'shape') {
    return 'rect'
  }
  if (normalized === 'roundedrectangle' || normalized === 'rounded-rectangle') {
    return 'roundRect'
  }
  if (normalized === 'circle' || normalized === 'oval') {
    return 'ellipse'
  }
  if (normalized === 'database' || normalized === 'cylinder' || normalized === 'flowchartmagneticdisk') {
    return 'flowChartMagneticDisk'
  }
  return value
}

function clamp01(value: number) {
  return Math.min(Math.max(value, 0), 1)
}

function parseFiniteFloat(value: string) {
  if (!/^[+-]?(?:(?:\d+(?:\.\d*)?)|(?:\.\d+))(?:[eE][+-]?\d+)?$/.test(value)) {
    return undefined
  }
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : undefined
}

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function invalidPresentation() {
  return new ApiError(
    422,
    'invalid_presentation_json',
    'The JSON could not be converted into a PowerPoint presentation.',
  )
}
