import type {
  NormalizedPresentation,
  NormalizedTextRun,
  XmlNode,
} from '../shared/PowerpointTypes'
import type {
  ExtractedSlideRecord,
  ExtractedTextBodyRecord,
  ExtractedTextRunRecord,
  ThemeTypography,
} from './PowerpointImportTypes'
import { cleanHex } from '../shared/PowerpointUtils'
import { positiveInt } from './PowerpointImportUtils'
import { child, children, findDescendant } from './PowerpointXml'
import {
  DEFAULT_FONT_FACE,
  DEFAULT_THEME,
  OOXML_FONT_SIZE_SCALE,
} from '../shared/PowerpointConstants'

const DARK_COLOR_RED_WEIGHT = 0.299
const DARK_COLOR_GREEN_WEIGHT = 0.587
const DARK_COLOR_BLUE_WEIGHT = 0.114
const DARK_COLOR_LUMINANCE_THRESHOLD = 145

export function extractText(textNode: XmlNode | undefined, fallbackTextNodes: Array<XmlNode | undefined> = []) {
  if (!textNode) {
    return undefined
  }

  const paragraphs = children(textNode, 'a:p').map((paragraph, paragraphIndex) => {
    const paragraphProperties = child(paragraph, 'a:pPr')
    const paragraphLevel = Math.min(Math.max(Number(paragraphProperties?.attributes?.lvl) || 0, 0), 8)
    const fallbackParagraphProperties = fallbackTextNodes.flatMap((node) =>
      paragraphPropertySources(node, paragraphIndex, paragraphLevel),
    )
    const primaryParagraphProperties = paragraphPropertySources(
      textNode,
      paragraphIndex,
      paragraphLevel,
    )
    const defaultRunProperties = {
      ...mergeRunProperties(fallbackParagraphProperties),
      ...mergeRunProperties(primaryParagraphProperties),
      ...extractRunProperties(child(paragraphProperties, 'a:defRPr')),
    }
    const runs = (paragraph.children ?? [])
      .filter((node) => node.tag === 'a:r' || node.tag === 'a:fld' || node.tag === 'a:br')
      .map((run) => ({
        text: run.tag === 'a:br' ? '\n' : child(run, 'a:t')?.text ?? '',
        properties: {
          ...defaultRunProperties,
          ...extractRunProperties(child(run, 'a:rPr')),
        },
      }))
    const bulletPrefix = extractBulletPrefix(paragraphProperties, paragraphIndex)
    if (bulletPrefix) {
      if (runs.length) {
        runs[0].text = `${bulletPrefix}${runs[0].text}`
      } else {
        runs.push({ text: bulletPrefix.trimEnd(), properties: defaultRunProperties })
      }
    }

    return {
      runs,
      properties: {
        ...mergeAttributes(fallbackParagraphProperties),
        ...mergeAttributes(primaryParagraphProperties),
        ...(paragraphProperties?.attributes ?? {}),
      },
      endParagraphRunProperties: {
        ...mergeEndParagraphRunProperties(fallbackTextNodes, paragraphIndex),
        ...(child(paragraph, 'a:endParaRPr')?.attributes ?? {}),
      },
    }
  })
  const plainText = paragraphs
    .map((paragraph) => paragraph.runs.map((run) => run.text).join(''))
    .join('\n')

  return {
    plainText,
    paragraphs,
    bodyProperties: {
      ...fallbackTextNodes.reduce<Record<string, string>>(
        (properties, node) => ({ ...properties, ...(child(node, 'a:bodyPr')?.attributes ?? {}) }),
        {},
      ),
      ...(child(textNode, 'a:bodyPr')?.attributes ?? {}),
    },
  }
}

function paragraphPropertySources(
  textNode: XmlNode | undefined,
  paragraphIndex: number,
  paragraphLevel: number,
) {
  if (!textNode) {
    return []
  }

  const levelTag = `a:lvl${paragraphLevel + 1}pPr`
  const listStyle = child(textNode, 'a:lstStyle')
  const levelProperties = child(listStyle, levelTag) ?? child(textNode, levelTag)
  const paragraphs = children(textNode, 'a:p')
  const directProperties = child(paragraphs[paragraphIndex] ?? paragraphs[0], 'a:pPr')
  return [levelProperties, directProperties].filter((node): node is XmlNode => !!node)
}

function mergeRunProperties(paragraphProperties: XmlNode[]) {
  return paragraphProperties.reduce<Record<string, string>>(
    (properties, node) => ({
      ...properties,
      ...extractRunProperties(child(node, 'a:defRPr')),
    }),
    {},
  )
}

function mergeAttributes(nodes: XmlNode[]) {
  return nodes.reduce<Record<string, string>>(
    (attributes, node) => ({ ...attributes, ...(node.attributes ?? {}) }),
    {},
  )
}

function mergeEndParagraphRunProperties(
  textNodes: Array<XmlNode | undefined>,
  paragraphIndex: number,
) {
  return textNodes.reduce<Record<string, string>>((properties, textNode) => {
    const paragraphs = children(textNode, 'a:p')
    const paragraph = paragraphs[paragraphIndex] ?? paragraphs[0]
    return {
      ...properties,
      ...(child(paragraph, 'a:endParaRPr')?.attributes ?? {}),
    }
  }, {})
}

function extractBulletPrefix(paragraphProperties: XmlNode | undefined, paragraphIndex: number) {
  if (!paragraphProperties || child(paragraphProperties, 'a:buNone')) {
    return ''
  }

  const bullet = child(paragraphProperties, 'a:buChar')
  if (bullet?.attributes?.char) {
    return `${bullet.attributes.char} `
  }

  const automatic = child(paragraphProperties, 'a:buAutoNum')
  if (automatic) {
    const startAt = positiveInt(automatic.attributes?.startAt, 1)
    const value = startAt + paragraphIndex
    const type = automatic.attributes?.type ?? ''
    return type.includes('ParenR') ? `${value}) ` : `${value}. `
  }

  return ''
}

export function applyExtractedTypography(presentation: NormalizedPresentation, extracted: ExtractedSlideRecord) {
  const theme = extractThemeTypography(extracted.supportParts)
  const sourceByPath = new Map(
    (extracted.shapeTree?.elements ?? [])
      .map((element) => [String(element.path ?? ''), element] as const)
      .filter(([sourcePath]) => !!sourcePath),
  )

  for (const slide of presentation.slides) {
    if (extracted.backgroundColor) {
      slide.backgroundColor = extracted.backgroundColor
    }

    for (const element of slide.elements) {
      if (element.kind === 'line' || element.kind === 'image') {
        continue
      }

      const source = sourceByPath.get(element.sourcePath)
      const extractedRuns = flattenExtractedTextRuns(source?.text)
      const fallbackFontFace = bestFontFace(extractedRuns, theme) || theme.bodyFont || element.fontFace

      if (element.kind === 'text') {
        element.fontFace = fallbackFontFace
        element.runs = applyRunTypography(element.runs, extractedRuns, theme, fallbackFontFace)
        element.color = firstRunColor(element.runs) || element.color
        element.fontSize = firstRunFontSize(element.runs) || element.fontSize
        element.bold = element.runs.length > 0 && element.runs.every((run) => run.bold)
        continue
      }

      element.fontFace = fallbackFontFace
      element.textRuns = applyRunTypography(element.textRuns, extractedRuns, theme, fallbackFontFace)
      element.textColor = firstRunColor(element.textRuns) || element.textColor
      element.fontSize = firstRunFontSize(element.textRuns) || element.fontSize
      element.bold = element.textRuns.length > 0 && element.textRuns.every((run) => run.bold)
    }
  }
}

function applyRunTypography(
  runs: NormalizedTextRun[],
  extractedRuns: ExtractedTextRunRecord[],
  theme: ThemeTypography,
  fallbackFontFace: string,
) {
  if (!runs.length) {
    return runs
  }

  return runs.map((run, index) => {
    const extracted = extractedRuns[index] ?? extractedRuns.find((candidate) => candidate.text === run.text)
    const properties = extracted?.properties ?? {}
    return {
      ...run,
      color: resolveRunColor(properties, theme) || run.color,
      fontFace: resolveRunFontFace(properties, theme) || fallbackFontFace || run.fontFace,
      fontSize: resolveRunFontSize(properties) || run.fontSize,
    }
  })
}

function extractRunProperties(runProperties: XmlNode | undefined) {
  const properties: Record<string, string> = { ...(runProperties?.attributes ?? {}) }
  const typeface = extractRunTypeface(runProperties)
  const color = extractRunColor(runProperties)

  if (typeface) {
    properties.fontFace = typeface
  }

  if (color.value) {
    properties.fontColor = color.value
  }

  if (color.scheme) {
    properties.fontSchemeColor = color.scheme
  }

  return properties
}

function flattenExtractedTextRuns(text: ExtractedTextBodyRecord | undefined): ExtractedTextRunRecord[] {
  return (text?.paragraphs ?? []).flatMap((paragraph) => paragraph.runs ?? [])
}

function bestFontFace(runs: ExtractedTextRunRecord[], theme: ThemeTypography) {
  for (const run of runs) {
    const fontFace = resolveRunFontFace(run.properties ?? {}, theme)
    if (fontFace) {
      return fontFace
    }
  }

  return theme.bodyFont
}

function firstRunColor(runs: NormalizedTextRun[]) {
  return runs.find((run) => !!run.color)?.color
}

function firstRunFontSize(runs: NormalizedTextRun[]) {
  return runs.find((run) => run.fontSize > 0)?.fontSize
}

function resolveRunFontFace(properties: Record<string, string>, theme: ThemeTypography) {
  const rawFontFace = properties.fontFace || properties.typeface

  if (!rawFontFace) {
    return theme.bodyFont
  }

  if (rawFontFace === '+mn-lt' || rawFontFace === '+mn-ea' || rawFontFace === '+mn-cs') {
    return theme.bodyFont
  }

  if (rawFontFace === '+mj-lt' || rawFontFace === '+mj-ea' || rawFontFace === '+mj-cs') {
    return theme.headingFont
  }

  return rawFontFace
}

function resolveRunColor(properties: Record<string, string>, theme: ThemeTypography) {
  if (properties.fontColor) {
    return cleanHex(properties.fontColor)
  }

  if (properties.fontSchemeColor) {
    const scheme = mapSchemeColorKey(properties.fontSchemeColor)
    return cleanHex(theme.colors[scheme] || theme.colors[properties.fontSchemeColor] || schemeColorFallback(scheme))
  }

  return undefined
}

function resolveRunFontSize(properties: Record<string, string>) {
  const size = Number(properties.sz)
  return Number.isFinite(size) && size > 0 ? size / OOXML_FONT_SIZE_SCALE : undefined
}

function extractRunTypeface(runProperties: XmlNode | undefined) {
  const latin = child(runProperties, 'a:latin') ?? child(runProperties, 'a:ea') ?? child(runProperties, 'a:cs')
  return latin?.attributes?.typeface
}

function extractRunColor(runProperties: XmlNode | undefined) {
  return extractFillColorValue(child(runProperties, 'a:solidFill'))
}

function extractFillColorValue(fillNode: XmlNode | undefined) {
  const colorNode = cloneColorNode(fillNode)

  if (!colorNode) {
    return {}
  }

  if (colorNode.tag === 'a:srgbClr') {
    return { value: colorNode.attributes?.val }
  }

  if (colorNode.tag === 'a:sysClr') {
    return { value: colorNode.attributes?.lastClr || colorNode.attributes?.val }
  }

  if (colorNode.tag === 'a:schemeClr') {
    return { scheme: colorNode.attributes?.val }
  }

  return {}
}

function extractThemeTypography(supportParts: ExtractedSlideRecord['supportParts']): ThemeTypography {
  const rawXml = Object.values(supportParts ?? {}).find(
    (part) => part.relationshipType.includes('/theme') && part.rawXml,
  )?.rawXml ?? supportParts?.['ppt/theme/theme1.xml']?.rawXml
  const fallback = {
    bodyFont: DEFAULT_FONT_FACE,
    headingFont: DEFAULT_FONT_FACE,
    colors: { ...DEFAULT_THEME },
  }

  if (!rawXml) {
    return fallback
  }

  return {
    bodyFont: extractThemeFont(rawXml, 'minorFont') || fallback.bodyFont,
    headingFont: extractThemeFont(rawXml, 'majorFont') || fallback.headingFont,
    colors: extractThemeColors(rawXml),
  }
}

function extractThemeFont(rawXml: string, fontKind: 'majorFont' | 'minorFont') {
  const match = rawXml.match(new RegExp(`<a:${fontKind}>[\\s\\S]*?<a:latin[^>]*typeface="([^"]*)"`))
  const typeface = match?.[1]?.trim()
  return typeface || undefined
}

function extractThemeColors(rawXml: string) {
  const colors: Record<string, string> = { ...DEFAULT_THEME }

  for (const key of Object.keys(colors)) {
    const match = rawXml.match(
      new RegExp(
        `<a:${key}>[\\s\\S]*?(?:<a:srgbClr val="([0-9A-Fa-f]{6})"\\/?>(?:[\\s\\S]*?)<\\/a:srgbClr>|<a:srgbClr val="([0-9A-Fa-f]{6})"\\s*\\/?>|<a:sysClr[^>]*lastClr="([0-9A-Fa-f]{6})"\\s*\\/?>)[\\s\\S]*?<\\/a:${key}>`,
      ),
    )
    const color = match?.[1] || match?.[2] || match?.[3]
    if (color) {
      colors[key] = color.toUpperCase()
    }
  }

  return colors
}

function mapSchemeColorKey(value: string) {
  const aliases: Record<string, string> = {
    bg1: 'lt1',
    tx1: 'dk1',
    bg2: 'lt2',
    tx2: 'dk2',
  }
  return aliases[value] || value
}

export function extractBackgroundColor(root: XmlNode) {
  const background = findDescendant(root, 'p:bg')
  const solidFill = findDescendant(background, 'a:solidFill')
  const color = extractFillColorValue(solidFill)

  if (color.value) {
    return cleanHex(color.value)
  }

  if (color.scheme) {
    return cleanHex(schemeColorFallback(mapSchemeColorKey(color.scheme)))
  }

  const backgroundRef = findDescendant(background, 'p:bgRef')
  const refColor = cloneColorNode(backgroundRef)

  if (refColor?.tag === 'a:srgbClr') {
    return cleanHex(refColor.attributes?.val)
  }

  if (refColor?.tag === 'a:sysClr') {
    return cleanHex(refColor.attributes?.lastClr || refColor.attributes?.val)
  }

  if (refColor?.tag === 'a:schemeClr') {
    return cleanHex(schemeColorFallback(mapSchemeColorKey(refColor.attributes?.val ?? '')))
  }

  return undefined
}

export function cloneColorNode(fillNode: XmlNode | undefined) {
  return cloneNode((fillNode?.children ?? []).find((candidate) => isColorTag(candidate.tag)))
}

export function cloneNode(node: XmlNode | undefined): XmlNode | undefined {
  if (!node) {
    return undefined
  }

  return {
    tag: node.tag,
    attributes: node.attributes ? { ...node.attributes } : undefined,
    text: node.text,
    children: node.children?.map((entry) => cloneNode(entry)).filter((entry): entry is XmlNode => !!entry),
  }
}

function isColorTag(tag: string) {
  return tag === 'a:srgbClr' || tag === 'a:sysClr' || tag === 'a:schemeClr'
}

export function isDarkHex(value: string) {
  const normalized = value.replace(/[^0-9A-Fa-f]/g, '').slice(0, 6)
  if (normalized.length !== 6) {
    return false
  }

  const red = Number.parseInt(normalized.slice(0, 2), 16)
  const green = Number.parseInt(normalized.slice(2, 4), 16)
  const blue = Number.parseInt(normalized.slice(4, 6), 16)
  const luminance =
    red * DARK_COLOR_RED_WEIGHT +
    green * DARK_COLOR_GREEN_WEIGHT +
    blue * DARK_COLOR_BLUE_WEIGHT
  return luminance < DARK_COLOR_LUMINANCE_THRESHOLD
}

export function schemeColorFallback(value: string | undefined) {
  const fallback: Record<string, string> = {
    dk1: '000000',
    dk2: '1F2937',
    bg1: 'FFFFFF',
    bg2: 'F3F4F6',
    lt1: 'FFFFFF',
    lt2: 'F3F4F6',
    tx1: '111827',
    tx2: '374151',
    accent1: '0B075B',
    accent2: 'F97316',
    accent3: '22C55E',
    accent4: 'FACC15',
    accent5: '2563EB',
    accent6: '9333EA',
  }
  return value ? fallback[value] : undefined
}
