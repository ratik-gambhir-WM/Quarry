export type JsonPrimitive = boolean | null | number | string
export type JsonValue = JsonPrimitive | JsonObject | JsonValue[]
export type JsonObject = {
  [key: string]: JsonValue | undefined
}

export type DiligenceTextRun = JsonObject & {
  bold?: boolean
  breakLine?: boolean
  color: string
  fontFace: string
  fontSize: number
  italic?: boolean
  text: string
  underline?: boolean
}

type DiligenceElementBase = JsonObject & {
  id: string
  opacity?: number
  rotate?: number
}

type DiligenceBoxElement = DiligenceElementBase & {
  align?: 'center' | 'left' | 'right'
  bold?: boolean
  borderRadius?: number
  fill: string
  fillOpacity?: number
  flipH?: boolean
  flipV?: boolean
  fontFace?: string
  fontSize?: number
  h: number
  italic?: boolean
  padding?: number
  runs?: DiligenceTextRun[]
  stroke: string
  strokeOpacity?: number
  strokeWidth: number
  text?: string
  textColor?: string
  valign?: 'bottom' | 'middle' | 'top'
  w: number
  x: number
  y: number
}

export type DiligenceShapeElement = DiligenceBoxElement & {
  shape: string
  type: 'shape'
}

export type DiligenceTextElement = DiligenceBoxElement & {
  type: 'text'
}

export type DiligenceLineElement = DiligenceElementBase & {
  beginArrow?: 'arrow' | 'diamond' | 'none' | 'oval' | 'stealth' | 'triangle'
  bendPosition?: number
  dash?: 'dash' | 'dot' | 'solid'
  endArrow?: 'arrow' | 'diamond' | 'none' | 'oval' | 'stealth' | 'triangle'
  lineType?: 'elbow' | 'straight'
  stroke: string
  strokeOpacity?: number
  strokeWidth: number
  type: 'line'
  x1: number
  x2: number
  y1: number
  y2: number
}

export type DiligenceImageElement = DiligenceElementBase & {
  altText?: string
  borderRadius?: number
  crop?: JsonObject & {
    bottom?: number
    left?: number
    right?: number
    top?: number
  }
  fit: 'contain' | 'cover' | 'stretch'
  flipH?: boolean
  flipV?: boolean
  h: number
  src: string
  type: 'image'
  w: number
  x: number
  y: number
}

export type DiligenceElement =
  | DiligenceImageElement
  | DiligenceLineElement
  | DiligenceShapeElement
  | DiligenceTextElement

export type DiligenceSlide = JsonObject & {
  backgroundColor: string
  elements: DiligenceElement[]
  height: number
  id: string
  name: string
  width: number
}

export type DiligencePresentation = JsonObject & {
  preserveElementOrder: boolean
  showBranding: boolean
  slides: DiligenceSlide[]
  title: string
}

/** The exact JSON body returned by /api/v1/import and accepted by /api/v1/export. */
export type DiligenceCanvasDocument = JsonObject & {
  presentation: DiligencePresentation
}

export function parseDiligenceCanvasDocument(value: unknown): DiligenceCanvasDocument {
  const root = requireRecord(value, 'response')
  const presentation = requireRecord(root.presentation, 'presentation')
  requireString(presentation.title, 'presentation.title')
  requireBoolean(presentation.preserveElementOrder, 'presentation.preserveElementOrder')
  requireBoolean(presentation.showBranding, 'presentation.showBranding')

  if (!Array.isArray(presentation.slides)) {
    throw new TypeError('presentation.slides must be an array.')
  }
  presentation.slides.forEach(validateSlide)

  return root as DiligenceCanvasDocument
}

function validateSlide(value: unknown, slideIndex: number) {
  const path = `presentation.slides[${slideIndex}]`
  const slide = requireRecord(value, path)
  requireNonEmptyString(slide.id, `${path}.id`)
  requireString(slide.name, `${path}.name`)
  requirePositiveNumber(slide.width, `${path}.width`)
  requirePositiveNumber(slide.height, `${path}.height`)
  requireNonEmptyString(slide.backgroundColor, `${path}.backgroundColor`)
  if (!Array.isArray(slide.elements)) {
    throw new TypeError(`${path}.elements must be an array.`)
  }
  slide.elements.forEach((element, elementIndex) =>
    validateElement(element, `${path}.elements[${elementIndex}]`),
  )
}

function validateElement(value: unknown, path: string) {
  const element = requireRecord(value, path)
  requireNonEmptyString(element.id, `${path}.id`)
  requireOptionalRange(element.opacity, `${path}.opacity`, 0, 1)
  requireOptionalNumber(element.rotate, `${path}.rotate`)
  const type = requireString(element.type, `${path}.type`)

  if (type === 'line') {
    requireNumber(element.x1, `${path}.x1`)
    requireNumber(element.y1, `${path}.y1`)
    requireNumber(element.x2, `${path}.x2`)
    requireNumber(element.y2, `${path}.y2`)
    requireString(element.stroke, `${path}.stroke`)
    requireNonNegativeNumber(element.strokeWidth, `${path}.strokeWidth`)
    requireOptionalRange(element.strokeOpacity, `${path}.strokeOpacity`, 0, 1)
    requireOptionalRange(element.bendPosition, `${path}.bendPosition`, 0, 1)
    requireOptionalEnum(
      element.lineType,
      `${path}.lineType`,
      ['elbow', 'straight'],
    )
    requireOptionalEnum(element.dash, `${path}.dash`, ['dash', 'dot', 'solid'])
    requireOptionalEnum(
      element.beginArrow,
      `${path}.beginArrow`,
      ['arrow', 'diamond', 'none', 'oval', 'stealth', 'triangle'],
    )
    requireOptionalEnum(
      element.endArrow,
      `${path}.endArrow`,
      ['arrow', 'diamond', 'none', 'oval', 'stealth', 'triangle'],
    )
    return
  }

  if (type === 'image') {
    validateBox(element, path)
    requireNonEmptyString(element.src, `${path}.src`)
    if (element.fit !== 'contain' && element.fit !== 'cover' && element.fit !== 'stretch') {
      throw new TypeError(`${path}.fit must be contain, cover, or stretch.`)
    }
    requireOptionalString(element.altText, `${path}.altText`)
    requireOptionalNonNegativeNumber(element.borderRadius, `${path}.borderRadius`)
    validateOptionalCrop(element.crop, `${path}.crop`)
    return
  }

  if (type === 'shape' || type === 'text') {
    validateBox(element, path)
    requireString(element.fill, `${path}.fill`)
    requireString(element.stroke, `${path}.stroke`)
    requireNonNegativeNumber(element.strokeWidth, `${path}.strokeWidth`)
    requireOptionalRange(element.fillOpacity, `${path}.fillOpacity`, 0, 1)
    requireOptionalRange(element.strokeOpacity, `${path}.strokeOpacity`, 0, 1)
    requireOptionalNonNegativeNumber(element.borderRadius, `${path}.borderRadius`)
    requireOptionalNonNegativeNumber(element.padding, `${path}.padding`)
    requireOptionalPositiveNumber(element.fontSize, `${path}.fontSize`)
    requireOptionalString(element.fontFace, `${path}.fontFace`)
    requireOptionalString(element.text, `${path}.text`)
    requireOptionalString(element.textColor, `${path}.textColor`)
    requireOptionalBoolean(element.bold, `${path}.bold`)
    requireOptionalBoolean(element.italic, `${path}.italic`)
    requireOptionalEnum(element.align, `${path}.align`, ['center', 'left', 'right'])
    requireOptionalEnum(element.valign, `${path}.valign`, ['bottom', 'middle', 'top'])
    validateOptionalRuns(element.runs, `${path}.runs`)
    if (type === 'shape') {
      requireNonEmptyString(element.shape, `${path}.shape`)
    }
    return
  }

  throw new TypeError(`${path}.type is not a supported element type.`)
}

function validateBox(element: Record<string, unknown>, path: string) {
  requireNumber(element.x, `${path}.x`)
  requireNumber(element.y, `${path}.y`)
  requirePositiveNumber(element.w, `${path}.w`)
  requirePositiveNumber(element.h, `${path}.h`)
  requireOptionalBoolean(element.flipH, `${path}.flipH`)
  requireOptionalBoolean(element.flipV, `${path}.flipV`)
}

function validateOptionalRuns(value: unknown, path: string) {
  if (value === undefined) {
    return
  }
  if (!Array.isArray(value)) {
    throw new TypeError(`${path} must be an array.`)
  }
  value.forEach((runValue, index) => {
    const runPath = `${path}[${index}]`
    const run = requireRecord(runValue, runPath)
    requireString(run.text, `${runPath}.text`)
    requireNonEmptyString(run.color, `${runPath}.color`)
    requireNonEmptyString(run.fontFace, `${runPath}.fontFace`)
    requirePositiveNumber(run.fontSize, `${runPath}.fontSize`)
    requireOptionalBoolean(run.bold, `${runPath}.bold`)
    requireOptionalBoolean(run.italic, `${runPath}.italic`)
    requireOptionalBoolean(run.underline, `${runPath}.underline`)
    requireOptionalBoolean(run.breakLine, `${runPath}.breakLine`)
  })
}

function validateOptionalCrop(value: unknown, path: string) {
  if (value === undefined) {
    return
  }
  const crop = requireRecord(value, path)
  requireOptionalRange(crop.top, `${path}.top`, 0, 1)
  requireOptionalRange(crop.right, `${path}.right`, 0, 1)
  requireOptionalRange(crop.bottom, `${path}.bottom`, 0, 1)
  requireOptionalRange(crop.left, `${path}.left`, 0, 1)
}

function requireRecord(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new TypeError(`${path} must be an object.`)
  }
  return value as Record<string, unknown>
}

function requireString(value: unknown, path: string): string {
  if (typeof value !== 'string') {
    throw new TypeError(`${path} must be a string.`)
  }
  return value
}

function requireNonEmptyString(value: unknown, path: string) {
  const stringValue = requireString(value, path)
  if (!stringValue.trim()) {
    throw new TypeError(`${path} cannot be empty.`)
  }
}

function requireBoolean(value: unknown, path: string) {
  if (typeof value !== 'boolean') {
    throw new TypeError(`${path} must be a boolean.`)
  }
}

function requireOptionalBoolean(value: unknown, path: string) {
  if (value !== undefined) {
    requireBoolean(value, path)
  }
}

function requireOptionalString(value: unknown, path: string) {
  if (value !== undefined) {
    requireString(value, path)
  }
}

function requireOptionalEnum(
  value: unknown,
  path: string,
  allowed: readonly string[],
) {
  if (value !== undefined && (typeof value !== 'string' || !allowed.includes(value))) {
    throw new TypeError(`${path} contains an unsupported value.`)
  }
}

function requireNumber(value: unknown, path: string) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new TypeError(`${path} must be a finite number.`)
  }
}

function requireOptionalNumber(value: unknown, path: string) {
  if (value !== undefined) {
    requireNumber(value, path)
  }
}

function requireOptionalRange(
  value: unknown,
  path: string,
  minimum: number,
  maximum: number,
) {
  if (value === undefined) {
    return
  }
  requireNumber(value, path)
  if ((value as number) < minimum || (value as number) > maximum) {
    throw new TypeError(`${path} must be between ${minimum} and ${maximum}.`)
  }
}

function requirePositiveNumber(value: unknown, path: string) {
  requireNumber(value, path)
  if ((value as number) <= 0) {
    throw new TypeError(`${path} must be greater than zero.`)
  }
}

function requireNonNegativeNumber(value: unknown, path: string) {
  requireNumber(value, path)
  if ((value as number) < 0) {
    throw new TypeError(`${path} cannot be negative.`)
  }
}

function requireOptionalPositiveNumber(value: unknown, path: string) {
  if (value !== undefined) {
    requirePositiveNumber(value, path)
  }
}

function requireOptionalNonNegativeNumber(value: unknown, path: string) {
  if (value !== undefined) {
    requireNonNegativeNumber(value, path)
  }
}
