export type JsonPrimitive = boolean | null | number | string
export type JsonValue = JsonPrimitive | JsonObject | JsonValue[]
export type JsonObject = {
  [key: string]: JsonValue | undefined
}
export type ThrownValue = bigint | boolean | null | number | object | string | symbol | undefined

export type XmlNode = {
  tag: string
  attributes?: Record<string, string>
  children?: XmlNode[]
  text?: string
}

export type ExtractedRelationship = {
  Id: string
  Type?: string
  Target?: string
  resolvedTarget?: string
  typeShort?: string
}

export interface ExtractedSupportPart {
  path?: string
  size?: number
  relationshipType?: string
  contentTypeHint?: string
  base64?: string
  rawXml?: string
}

export interface ExtractedTransform {
  xPx?: number
  yPx?: number
  widthPx?: number
  heightPx?: number
  rotation?: number | null
  xInches?: number
  yInches?: number
  widthInches?: number
  heightInches?: number
  flipH?: boolean
  flipV?: boolean
}

export interface ExtractedTextRun {
  text?: string
  properties?: Record<string, string>
}

export interface ExtractedTextParagraph {
  runs?: ExtractedTextRun[]
  properties?: Record<string, string>
  endParagraphRunProperties?: Record<string, string>
}

export interface ExtractedTextBody {
  plainText?: string
  paragraphs?: ExtractedTextParagraph[]
  bodyProperties?: Record<string, string>
}

export interface ExtractedShapeElement {
  path?: string
  zIndex?: number
  tag?: string
  kind?: string
  nonVisual?: {
    id?: number
    name?: string
    description?: string
    hidden?: boolean
    isTextBox?: boolean
  }
  transform?: ExtractedTransform
  presetGeometry?: {
    preset?: string
    xmlAst?: XmlNode
  }
  relationshipIds?: string[]
  xmlAst?: XmlNode
  text?: ExtractedTextBody
  shapeProperties?: XmlNode
  style?: XmlNode
}

export interface ExtractedSlideSpec {
  slideNumber?: number
  slideSize?: {
    widthPx?: number
    heightPx?: number
    widthInches?: number
    heightInches?: number
  }
  relationships?: ExtractedRelationship[]
  shapeTree?: {
    elements?: ExtractedShapeElement[]
  }
  summary?: {
    totalDrawableElements?: number
    textElementCount?: number
  }
  supportParts?: Record<string, ExtractedSupportPart>
}

export type VerticalAlign = 'top' | 'middle' | 'bottom'
export type HorizontalAlign = 'left' | 'center' | 'right'
export type DashStyle = 'solid' | 'dash' | 'dot'

export interface ValidationIssue {
  level: 'error' | 'warning'
  path: string
  message: string
}

export interface NormalizedTextRun {
  text: string
  bold: boolean
  italic: boolean
  underline: boolean
  color: string
  fontFace: string
  fontSize: number
  breakLine?: boolean
}

export interface BaseElement {
  id: string
  sourcePath: string
  opacity: number
  rotate: number
  flipH?: boolean
  flipV?: boolean
  valign: VerticalAlign
}

export interface NormalizedTextElement extends BaseElement {
  kind: 'text'
  x: number
  y: number
  w: number
  h: number
  text: string
  fill: string
  fillOpacity?: number
  stroke: string
  strokeOpacity?: number
  strokeWidth: number
  borderRadius: number
  padding: number
  align: HorizontalAlign
  color: string
  fontSize: number
  fontFace: string
  bold: boolean
  italic: boolean
  runs: NormalizedTextRun[]
}

export interface NormalizedShapeElement extends BaseElement {
  kind: 'shape'
  x: number
  y: number
  w: number
  h: number
  shape: string
  label: string
  fill: string
  fillOpacity?: number
  stroke: string
  strokeOpacity?: number
  strokeWidth: number
  borderRadius: number
  padding: number
  align: HorizontalAlign
  textColor: string
  fontSize: number
  fontFace: string
  bold: boolean
  textRuns: NormalizedTextRun[]
}

export interface NormalizedLineElement extends BaseElement {
  kind: 'line'
  lineType: 'straight' | 'elbow'
  elbowDirection?: 'horizontal-first' | 'vertical-first'
  x1: number
  y1: number
  x2: number
  y2: number
  stroke: string
  strokeOpacity?: number
  strokeWidth: number
  dash: DashStyle
  beginArrow?: 'none' | 'triangle' | 'arrow' | 'diamond' | 'oval' | 'stealth'
  endArrow: 'none' | 'triangle' | 'arrow' | 'diamond' | 'oval' | 'stealth'
  occlusionRects: LineOcclusionRect[]
}

export interface NormalizedImageElement extends BaseElement {
  kind: 'image'
  x: number
  y: number
  w: number
  h: number
  src: string
  fit: 'contain' | 'cover' | 'stretch'
  crop?: {
    bottom: number
    left: number
    right: number
    top: number
  }
  borderRadius: number
  altText: string
}

export type NormalizedElement =
  | NormalizedTextElement
  | NormalizedShapeElement
  | NormalizedLineElement
  | NormalizedImageElement

export interface LineOcclusionRect {
  x: number
  y: number
  w: number
  h: number
}

export interface NormalizedSlide {
  id: string
  name: string
  width: number
  height: number
  backgroundColor: string
  preserveElementOrder: boolean
  elements: NormalizedElement[]
}

export interface NormalizedPresentation {
  meta: {
    title: string
    width: number
    height: number
    preserveElementOrder: boolean
    showBranding: boolean
    sourceType: 'extracted-slide' | 'native-presentation'
  }
  slides: NormalizedSlide[]
}

export interface NormalizationOptions {
  baseDir?: string
}

export type SlideForElementLayering = Pick<NormalizedSlide, 'elements' | 'height' | 'width'>
