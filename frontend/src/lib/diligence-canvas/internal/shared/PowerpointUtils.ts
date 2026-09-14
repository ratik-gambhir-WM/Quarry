import { EMU_PER_INCH, PX_PER_INCH, shapeAliases } from './PowerpointConstants'
import type {
  JsonObject,
  DashStyle,
  HorizontalAlign,
  NormalizationOptions,
  NormalizedImageElement,
  NormalizedLineElement,
  VerticalAlign,
  XmlNode,
} from './PowerpointTypes'

export function parseLineColor(lineNode: XmlNode | undefined, theme: Record<string, string>, fallback: string) {
  if (!lineNode || hasChild(lineNode, 'a:noFill')) {
    return 'transparent'
  }

  return parseColor(findChild(lineNode, 'a:solidFill'), theme, fallback)
}

export function parseFontColor(styleNode: XmlNode | undefined, theme: Record<string, string>, fallback: string) {
  const fontRef = findChild(styleNode, 'a:fontRef')
  if (!fontRef) {
    return fallback
  }

  const colorNode = (fontRef.children ?? []).find((child) => isColorTag(child.tag))
  return colorNode ? parseColorNode(colorNode, theme, fallback) : fallback
}

export function parseColor(fillNode: XmlNode | undefined, theme: Record<string, string>, fallback: string) {
  if (!fillNode) {
    return fallback
  }

  const colorNode = (fillNode.children ?? []).find((child) => isColorTag(child.tag))
  return colorNode ? parseColorNode(colorNode, theme, fallback) : fallback
}

export function parseColorOpacity(fillNode: XmlNode | undefined) {
  const colorNode = fillNode?.children?.find((child) => isColorTag(child.tag))
  if (!colorNode) {
    return 1
  }

  let opacity = 1
  for (const modifier of colorNode.children ?? []) {
    const value = Number(modifier.attributes?.val ?? 100000) / 100000
    if (modifier.tag === 'a:alpha') {
      opacity = value
    } else if (modifier.tag === 'a:alphaMod') {
      opacity *= value
    } else if (modifier.tag === 'a:alphaOff') {
      opacity += value
    }
  }

  return clamp01(opacity)
}

export function parseColorNode(node: XmlNode, theme: Record<string, string>, fallback: string) {
  if (node.tag === 'a:srgbClr') {
    return cleanHex(node.attributes?.val, fallback)
  }

  if (node.tag === 'a:sysClr') {
    return cleanHex(node.attributes?.lastClr || node.attributes?.val, fallback)
  }

  if (node.tag === 'a:schemeClr') {
    const scheme = node.attributes?.val ?? ''
    const base = theme[mapSchemeColorKey(scheme)] || theme[scheme] || fallback
    return applyColorModifiers(cleanHex(base, fallback), node.children ?? [])
  }

  return fallback
}

export function applyColorModifiers(baseHex: string, children: XmlNode[]) {
  let rgb = hexToRgb(baseHex)
  if (!rgb) {
    return baseHex
  }

  for (const child of children) {
    if (child.tag === 'a:shade') {
      const factor = Number(child.attributes?.val ?? '100000') / 100000
      rgb = rgb.map((channel) => Math.round(channel * factor)) as [number, number, number]
    }

    if (child.tag === 'a:tint') {
      const factor = Number(child.attributes?.val ?? '0') / 100000
      rgb = rgb.map((channel) => Math.round(channel + (255 - channel) * factor)) as [
        number,
        number,
        number,
      ]
    }
  }

  return rgbToHex(rgb)
}

export function normalizeShapeName(shape: string | undefined) {
  if (!shape) {
    return 'rect'
  }

  return shapeAliases[shape] || shape
}

export function normalizeNativeKind(kind: string | undefined) {
  if (!kind) {
    return undefined
  }

  const lower = kind.toLowerCase()
  if (lower === 'text' || lower === 'textbox') {
    return 'text'
  }
  if (
    lower === 'image' ||
    lower === 'picture'
  ) {
    return 'image'
  }
  if (
    lower === 'line' ||
    lower === 'arrow' ||
    lower === 'connector'
  ) {
    return 'line'
  }
  return 'shape'
}

export function normalizeAlign(value: string | undefined): HorizontalAlign {
  if (value === 'ctr' || value === 'center') {
    return 'center'
  }

  if (value === 'r' || value === 'right') {
    return 'right'
  }

  return 'left'
}

export function normalizeValign(value: string | undefined): VerticalAlign {
  if (value === 'top' || value === 't') {
    return 'top'
  }

  if (value === 'bottom' || value === 'b') {
    return 'bottom'
  }

  return 'middle'
}

export function normalizeBodyAnchor(value: string | undefined): VerticalAlign {
  if (value === 't') {
    return 'top'
  }

  if (value === 'b') {
    return 'bottom'
  }

  return 'middle'
}

export function normalizeDash(value: string | undefined): DashStyle {
  if (value === 'dash' || value === 'sysDash') {
    return 'dash'
  }

  if (value === 'dot' || value === 'sysDot') {
    return 'dot'
  }

  return 'solid'
}

export function normalizeArrow(value: string | undefined): NormalizedLineElement['endArrow'] {
  if (
    value === 'triangle' ||
    value === 'arrow' ||
    value === 'diamond' ||
    value === 'oval' ||
    value === 'stealth'
  ) {
    return value
  }

  return 'none'
}

export function normalizeLineType(value: string | undefined): NormalizedLineElement['lineType'] {
  if (value === 'elbow' || value === 'angle' || value === 'angled' || value === 'angleBracket') {
    return 'elbow'
  }

  return 'straight'
}

export function parseDashStyle(lineNode: XmlNode | undefined): DashStyle {
  const dashNode = findChild(lineNode, 'a:prstDash')
  return normalizeDash(dashNode?.attributes?.val)
}

export function parseArrowType(lineNode: XmlNode | undefined): NormalizedLineElement['endArrow'] {
  const tailEnd = findChild(lineNode, 'a:tailEnd')
  return normalizeArrow(tailEnd?.attributes?.type)
}

export function parseBeginArrowType(lineNode: XmlNode | undefined): NormalizedLineElement['beginArrow'] {
  const headEnd = findChild(lineNode, 'a:headEnd')
  return normalizeArrow(headEnd?.attributes?.type)
}

export function bodyPadding(bodyProperties: Record<string, string> | undefined) {
  const rawInsets = [
    bodyProperties?.lIns,
    bodyProperties?.tIns,
    bodyProperties?.rIns,
    bodyProperties?.bIns,
  ]
  const definedInsets = rawInsets.filter((value): value is string => value !== undefined)
  if (!definedInsets.length) {
    return 8
  }

  const insets = definedInsets.map(emuToPoints)
  return insets.reduce((sum, value) => sum + value, 0) / insets.length
}

export function resolveImageSource(src: string | undefined, options: NormalizationOptions) {
  if (!src) {
    return undefined
  }

  if (src.startsWith('data:') || src.startsWith('http://') || src.startsWith('https://')) {
    return src
  }

  if (isAbsolutePathLike(src)) {
    return src
  }

  if (options.baseDir) {
    return joinPathLike(options.baseDir, src)
  }

  return src
}

export function normalizeImageFit(value: string | undefined): NormalizedImageElement['fit'] {
  if (value === 'cover' || value === 'stretch') {
    return value
  }

  return 'contain'
}

export function emuLineWidthToPoints(value: string | undefined) {
  const width = Number(value)
  if (!Number.isFinite(width) || width <= 0) {
    return 1
  }
  return width / 12700
}

export function emuToPoints(value: string | undefined) {
  const emu = Number(value)
  if (!Number.isFinite(emu) || emu <= 0) {
    return 0
  }
  return (emu / EMU_PER_INCH) * 72
}

export function inchesToPx(inches: number) {
  return inches * PX_PER_INCH
}

export function cleanHex(value: string | undefined): string | undefined
export function cleanHex(value: string | undefined, fallback: string): string
export function cleanHex(value: string | undefined, fallback?: string) {
  if (!value) {
    return fallback
  }

  const trimmed = value.replace('#', '').trim()
  if (trimmed.toLowerCase() === 'transparent') {
    return 'transparent'
  }

  return /^[0-9a-fA-F]{6}$/.test(trimmed) ? trimmed.toUpperCase() : fallback
}

function mapSchemeColorKey(key: string) {
  if (key === 'tx1') {
    return 'dk1'
  }
  if (key === 'bg1') {
    return 'lt1'
  }
  if (key === 'tx2') {
    return 'dk2'
  }
  if (key === 'bg2') {
    return 'lt2'
  }
  return key
}

function isColorTag(tag: string) {
  return tag === 'a:srgbClr' || tag === 'a:schemeClr' || tag === 'a:sysClr'
}

export function findChild(node: XmlNode | undefined, tag: string) {
  return (node?.children ?? []).find((child) => child.tag === tag)
}

export function hasChild(node: XmlNode | undefined, tag: string) {
  return !!findChild(node, tag)
}

export function isRecord<T>(value: T): value is T & JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

export function asString<T>(value: T) {
  return typeof value === 'string' ? value : undefined
}

export function coerceNumber<T>(value: T, fallback: number) {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback
}

export function coerceBoolean<T>(value: T) {
  return value === true || value === 'true' || value === 1 || value === '1'
}

export function clamp01(value: number) {
  return Math.max(0, Math.min(1, value))
}

export function clampNumber(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max)
}

function hexToRgb(hex: string) {
  const normalized = cleanHex(hex, '')
  if (!normalized) {
    return undefined
  }

  return [
    Number.parseInt(normalized.slice(0, 2), 16),
    Number.parseInt(normalized.slice(2, 4), 16),
    Number.parseInt(normalized.slice(4, 6), 16),
  ] as [number, number, number]
}

function rgbToHex([r, g, b]: [number, number, number]) {
  return [r, g, b]
    .map((value) => value.toString(16).padStart(2, '0'))
    .join('')
    .toUpperCase()
}

export function isDefined<T>(value: T | undefined): value is T {
  return value !== undefined
}

function isAbsolutePathLike(value: string) {
  return value.startsWith('/') || /^[A-Za-z]:[\\/]/.test(value)
}

function joinPathLike(baseDir: string, relativePath: string) {
  const separator = baseDir.includes('\\') ? '\\' : '/'
  const trimmedBase = baseDir.replace(/[\\/]+$/, '')
  const trimmedRelative = relativePath.replace(/^[\\/]+/, '')
  return `${trimmedBase}${separator}${trimmedRelative}`
}
