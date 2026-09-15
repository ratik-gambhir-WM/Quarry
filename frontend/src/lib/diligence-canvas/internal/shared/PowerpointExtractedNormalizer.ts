import {
  DEFAULT_FONT_FACE,
  DEFAULT_HEIGHT_PX,
  DEFAULT_THEME,
  DEFAULT_WIDTH_PX,
} from './PowerpointConstants'
import type {
  ExtractedRelationship,
  ExtractedShapeElement,
  ExtractedSlideSpec,
  ExtractedSupportPart,
  ExtractedTextBody,
  NormalizedElement,
  NormalizedImageElement,
  NormalizedPresentation,
  NormalizedTextRun,
  ValidationIssue,
  XmlNode,
} from './PowerpointTypes'
import {
  bodyPadding,
  clampNumber,
  coerceNumber,
  emuLineWidthToPoints,
  findChild,
  hasChild,
  normalizeAlign,
  normalizeBodyAnchor,
  normalizeShapeName,
  parseArrowType,
  parseBeginArrowType,
  parseColor,
  parseColorOpacity,
  parseDashStyle,
  parseFontColor,
  parseLineColor,
} from './PowerpointUtils'

export function normalizeExtractedPresentation(
  input: ExtractedSlideSpec,
  issues: ValidationIssue[],
): NormalizedPresentation {
  const width = coerceNumber(input.slideSize?.widthPx, DEFAULT_WIDTH_PX)
  const height = coerceNumber(input.slideSize?.heightPx, DEFAULT_HEIGHT_PX)
  const theme = extractThemeColors(input.supportParts)
  const slideId = `slide-${input.slideNumber ?? 1}`
  const slideName = deriveTitleFromExtractedSlide(input)
  const normalizedElements = (input.shapeTree?.elements ?? [])
    .slice()
    .sort((left, right) => coerceNumber(left.zIndex, 0) - coerceNumber(right.zIndex, 0))
    .flatMap((element, index) =>
      normalizeExtractedElement(
        element,
        index,
        width,
        height,
        theme,
        input.relationships ?? [],
        input.supportParts ?? {},
        issues,
      ),
    )
  const elements = normalizedElements

  if (!elements.length) {
    issues.push({
      level: 'warning',
      path: 'shapeTree.elements',
      message: 'No exportable slide elements were found. The PowerPoint will be empty.',
    })
  }

  return {
    meta: {
      title: slideName,
      width,
      height,
      preserveElementOrder: true,
      showBranding: false,
      sourceType: 'extracted-slide',
    },
    slides: [
      {
        id: slideId,
        name: slideName,
        width,
        height,
        backgroundColor: 'FFFFFF',
        preserveElementOrder: true,
        elements,
      },
    ],
  }
}

function normalizeExtractedElement(
  element: ExtractedShapeElement,
  index: number,
  slideWidth: number,
  slideHeight: number,
  theme: Record<string, string>,
  relationships: ExtractedRelationship[],
  supportParts: Record<string, ExtractedSupportPart>,
  issues: ValidationIssue[],
): NormalizedElement[] {
  if (element.nonVisual?.hidden) {
    return []
  }

  const id = `element-${element.nonVisual?.id ?? index + 1}`
  const sourcePath = element.path || `shapeTree.elements[${index}]`
  const transform = element.transform ?? {}
  const x = coerceNumber(transform.xPx, transform.xInches ? transform.xInches * 96 : 0)
  const y = coerceNumber(transform.yPx, transform.yInches ? transform.yInches * 96 : 0)
  const w = coerceNumber(transform.widthPx, transform.widthInches ? transform.widthInches * 96 : 0)
  const h = coerceNumber(transform.heightPx, transform.heightInches ? transform.heightInches * 96 : 0)
  const rotate = coerceNumber(transform.rotation, 0)
  const presetShape = normalizeShapeName(element.presetGeometry?.preset)

  if (
    element.kind === 'connector' ||
    presetShape === 'line' ||
    presetShape === 'lineInv' ||
    presetShape.startsWith('straightConnector') ||
    presetShape.startsWith('bentConnector')
  ) {
    const lineNode = findChild(element.shapeProperties, 'a:ln')
    const lineFill = findChild(lineNode, 'a:solidFill')
    const lineReference = findChild(element.style, 'a:lnRef')
    const reverseX = !!transform.flipH || presetShape === 'lineInv'
    const reverseY = !!transform.flipV || presetShape === 'lineInv'
    return [
      {
        kind: 'line',
        id,
        sourcePath,
        opacity: 1,
        rotate,
        valign: 'middle',
        lineType: presetShape.startsWith('bentConnector') ? 'elbow' : 'straight',
        bendPosition: getConnectorBendPosition(element.presetGeometry?.xmlAst),
        x1: clampNumber(reverseX ? x + w : x, 0, slideWidth),
        y1: clampNumber(reverseY ? y + h : y, 0, slideHeight),
        x2: clampNumber(reverseX ? x : x + w, 0, slideWidth),
        y2: clampNumber(reverseY ? y : y + h, 0, slideHeight),
        stroke: lineFill
          ? parseLineColor(lineNode, theme, '334155')
          : parseColor(lineReference, theme, '334155'),
        strokeOpacity: parseColorOpacity(lineFill ?? lineReference),
        strokeWidth: emuLineWidthToPoints(lineNode?.attributes?.w),
        dash: parseDashStyle(lineNode),
        beginArrow: parseBeginArrowType(lineNode),
        endArrow: parseArrowType(lineNode),
        occlusionRects: [],
      },
    ]
  }

  if (element.kind === 'graphicFrame') {
    const image = extractGraphicFrameImage(
      element,
      relationships,
      supportParts,
      theme,
      id,
      sourcePath,
      x,
      y,
      w,
      h,
      rotate,
    )
    if (image) {
      return [image]
    }

    issues.push({
      level: 'warning',
      path: sourcePath,
      message: 'Skipped a graphic frame because it is not a plain PowerPoint drawable element.',
    })
    return []
  }

  if (element.kind !== 'shape') {
    issues.push({
      level: 'warning',
      path: sourcePath,
      message: `Skipped unsupported extracted element kind "${element.kind ?? 'unknown'}".`,
    })
    return []
  }

  const textBody = element.text
  if (hasChild(element.shapeProperties, 'a:custGeom')) {
    issues.push({
      level: 'warning',
      path: sourcePath,
      message: 'Rendered a custom/freeform PowerPoint geometry as a rectangle fallback.',
    })
  }
  if (
    hasChild(element.shapeProperties, 'a:gradFill') ||
    hasChild(element.shapeProperties, 'a:pattFill') ||
    hasChild(element.shapeProperties, 'a:blipFill')
  ) {
    issues.push({
      level: 'warning',
      path: sourcePath,
      message: 'This shape uses a gradient, pattern, or picture fill; the compact canvas schema uses a solid-color fallback.',
    })
  }
  const textRuns = normalizeExtractedTextRuns(textBody, theme, element.style)
  const label = paragraphText(textBody)
  const fillNode = findChild(element.shapeProperties, 'a:solidFill')
  const lineNode = findChild(element.shapeProperties, 'a:ln')
  const fillReference = findChild(element.style, 'a:fillRef')
  const lineReference = findChild(element.style, 'a:lnRef')
  const hasLineReference = !!lineReference && lineReference.attributes?.idx !== '0'
  const lineFill = findChild(lineNode, 'a:solidFill')
  const hasNoFill = hasChild(element.shapeProperties, 'a:noFill')
  const fill = hasNoFill
    ? 'transparent'
    : fillNode
      ? parseColor(fillNode, theme, 'transparent')
      : parseColor(fillReference, theme, 'transparent')
  const strokeVisible =
    !hasChild(lineNode, 'a:noFill') &&
    (!!lineFill || hasLineReference)
  const stroke = strokeVisible
    ? lineFill
      ? parseLineColor(lineNode, theme, '334155')
      : parseColor(lineReference, theme, '334155')
    : 'transparent'
  const strokeWidth = strokeVisible ? emuLineWidthToPoints(lineNode?.attributes?.w) : 0
  const textColor =
    textRuns.find((run) => run.color)?.color ??
    parseFontColor(element.style, theme, stroke === 'transparent' ? '111827' : 'FFFFFF')
  const fontSize = textRuns.find((run) => run.fontSize)?.fontSize ?? 16
  const fontFace = textRuns.find((run) => run.fontFace)?.fontFace ?? DEFAULT_FONT_FACE
  const align = normalizeAlign(textBody?.paragraphs?.[0]?.properties?.algn)
  const valign = normalizeBodyAnchor(textBody?.bodyProperties?.anchor)
  const padding = bodyPadding(textBody?.bodyProperties)
  const shapeName = presetShape
  const fillOpacity = parseColorOpacity(fillNode ?? fillReference)
  const strokeOpacity = parseColorOpacity(lineFill ?? lineReference)

  if (!label && w <= 0 && h <= 0) {
    return []
  }

  const textOnly =
    !!label &&
    (element.nonVisual?.isTextBox ||
      element.nonVisual?.name?.toLowerCase().includes('textbox') ||
      ((fill === 'transparent' || fillOpacity === 0) &&
        (stroke === 'transparent' || strokeOpacity === 0)))

  if (textOnly) {
    return [
      {
        kind: 'text',
        id,
        sourcePath,
        opacity: 1,
        rotate,
        flipH: transform.flipH || undefined,
        flipV: transform.flipV || undefined,
        valign,
        x,
        y,
        w: Math.max(w, 1),
        h: Math.max(h, fontSize * Math.max(textRuns.length, 1)),
        text: label,
        fill: 'transparent',
        fillOpacity,
        stroke: 'transparent',
        strokeOpacity,
        strokeWidth: 0,
        borderRadius: 0,
        padding,
        align,
        color: textColor,
        fontSize,
        fontFace,
        bold: textRuns.length > 0 && textRuns.every((run) => run.bold),
        italic: textRuns.length > 0 && textRuns.every((run) => run.italic),
        runs: textRuns.length
          ? textRuns
          : [
              {
                text: label,
                bold: false,
                italic: false,
                underline: false,
                color: textColor,
                fontSize,
                fontFace,
              },
            ],
      },
    ]
  }

  return [
    {
      kind: 'shape',
      id,
      sourcePath,
      opacity: 1,
      rotate,
      flipH: transform.flipH || undefined,
      flipV: transform.flipV || undefined,
      valign,
      x,
      y,
      w,
      h,
      shape: shapeName,
      label,
      fill,
      fillOpacity,
      stroke,
      strokeOpacity,
      strokeWidth,
      borderRadius: parseBorderRadius(element, shapeName, w, h),
      padding,
      align,
      textColor,
      fontSize,
      fontFace,
      bold: textRuns.length > 0 && textRuns.every((run) => run.bold),
      textRuns: textRuns.length
        ? textRuns
        : label
          ? [
              {
                text: label,
                bold: false,
                italic: false,
                underline: false,
                color: textColor,
                fontSize,
                fontFace,
              },
            ]
          : [],
    },
  ]
}

function getConnectorBendPosition(geometry: XmlNode | undefined) {
  const adjustmentList = findChild(geometry, 'a:avLst')
  const adjustment = (adjustmentList?.children ?? []).find(
    (child) => child.tag === 'a:gd' && child.attributes?.name === 'adj1',
  )
  const value = adjustment?.attributes?.fmla?.match(/^val\s+(-?\d+(?:\.\d+)?)$/)?.[1]
  return clampNumber(coerceNumber(value === undefined ? undefined : Number(value), 50_000) / 100_000, 0, 1)
}

function parseBorderRadius(
  element: ExtractedShapeElement,
  shapeName: string,
  width: number,
  height: number,
) {
  if (shapeName !== 'roundRect') {
    return 0
  }

  const adjustment = findChild(findChild(element.presetGeometry?.xmlAst, 'a:avLst'), 'a:gd')
  const match = /^val\s+(-?\d+(?:\.\d+)?)$/u.exec(adjustment?.attributes?.fmla ?? '')
  const ratio = match ? clampNumber(Number(match[1]) / 100000, 0, 0.5) : 1 / 6
  return Math.min(width, height) * ratio
}

function extractGraphicFrameImage(
  element: ExtractedShapeElement,
  relationships: ExtractedRelationship[],
  supportParts: Record<string, ExtractedSupportPart>,
  theme: Record<string, string>,
  id: string,
  sourcePath: string,
  x: number,
  y: number,
  w: number,
  h: number,
  rotate: number,
) {
  void theme
  const relationshipId = element.relationshipIds?.find((candidate) => {
    const relationship = relationships.find((entry) => entry.Id === candidate)
    return relationship?.typeShort === 'image' || relationship?.Type?.includes('/image')
  })

  if (!relationshipId) {
    return undefined
  }

  const relationship = relationships.find((entry) => entry.Id === relationshipId)
  const supportPart = relationship ? getRelationshipSupportPart(relationship, supportParts) : undefined
  const base64 = supportPart?.base64
  const mimeType =
    supportPart?.contentTypeHint && supportPart.contentTypeHint.startsWith('image/')
      ? supportPart.contentTypeHint
      : extensionToMimeType((relationship?.resolvedTarget || relationship?.Target || '').split('.').pop())

  if (!relationship || !base64 || !mimeType) {
    return undefined
  }

  const imageElement: NormalizedImageElement = {
    kind: 'image',
    id,
    sourcePath,
    opacity: extractImageOpacity(element.xmlAst),
    rotate,
    flipH: element.transform?.flipH || undefined,
    flipV: element.transform?.flipV || undefined,
    valign: 'middle',
    x,
    y,
    w: Math.max(w, 1),
    h: Math.max(h, 1),
    src: `data:${mimeType};base64,${base64}`,
    fit: 'stretch',
    crop: extractImageCrop(element.xmlAst),
    borderRadius: 0,
    altText: element.nonVisual?.description || element.nonVisual?.name || 'Embedded image',
  }
  return imageElement
}

function extractImageOpacity(node: XmlNode | undefined) {
  const blip = findFirstDescendant(node, 'a:blip')
  const alpha = findChild(blip, 'a:alphaModFix')
  return clampNumber(Number(alpha?.attributes?.amt ?? 100000) / 100000, 0, 1)
}

function extractImageCrop(node: XmlNode | undefined): NormalizedImageElement['crop'] {
  const srcRect = findChild(findFirstDescendant(node, 'p:blipFill'), 'a:srcRect')
  if (!srcRect) {
    return undefined
  }

  const crop = {
    top: clampNumber(Number(srcRect.attributes?.t ?? 0) / 100000, 0, 1),
    right: clampNumber(Number(srcRect.attributes?.r ?? 0) / 100000, 0, 1),
    bottom: clampNumber(Number(srcRect.attributes?.b ?? 0) / 100000, 0, 1),
    left: clampNumber(Number(srcRect.attributes?.l ?? 0) / 100000, 0, 1),
  }

  return crop.top || crop.right || crop.bottom || crop.left ? crop : undefined
}

function findFirstDescendant(node: XmlNode | undefined, tag: string): XmlNode | undefined {
  if (!node) {
    return undefined
  }
  if (node.tag === tag) {
    return node
  }

  for (const childNode of node.children ?? []) {
    const match = findFirstDescendant(childNode, tag)
    if (match) {
      return match
    }
  }

  return undefined
}

function getRelationshipSupportPart(
  relationship: ExtractedRelationship,
  supportParts: Record<string, ExtractedSupportPart>,
) {
  const candidates = [
    relationship.resolvedTarget,
    relationship.Target,
  ].filter((candidate): candidate is string => !!candidate)

  for (const candidate of candidates) {
    const exactMatch = supportParts[candidate]
    if (exactMatch) {
      return exactMatch
    }

    const normalizedCandidate = candidate.replace(/^\/+/u, '')
    const normalizedMatch = Object.entries(supportParts).find(
      ([partPath]) => partPath.replace(/^\/+/u, '') === normalizedCandidate,
    )?.[1]
    if (normalizedMatch) {
      return normalizedMatch
    }
  }

  return undefined
}

function extensionToMimeType(extension: string | undefined) {
  if (!extension) {
    return undefined
  }

  const normalized = extension.toLowerCase()
  if (normalized === 'png') {
    return 'image/png'
  }
  if (normalized === 'jpg' || normalized === 'jpeg') {
    return 'image/jpeg'
  }
  if (normalized === 'svg') {
    return 'image/svg+xml'
  }
  if (normalized === 'gif') {
    return 'image/gif'
  }
  if (normalized === 'emf') {
    return 'image/emf'
  }

  return undefined
}

function normalizeExtractedTextRuns(
  textBody: ExtractedTextBody | undefined,
  theme: Record<string, string>,
  styleNode: XmlNode | undefined,
) {
  const runs: NormalizedTextRun[] = []
  const defaultColor = parseFontColor(styleNode, theme, '111827')

  for (const paragraph of textBody?.paragraphs ?? []) {
    const paragraphRuns = paragraph.runs ?? []

    if (!paragraphRuns.length) {
      continue
    }

    paragraphRuns.forEach((run, runIndex) => {
      const properties = run.properties ?? {}
      runs.push({
        text: run.text ?? '',
        bold: properties.b === '1',
        italic: properties.i === '1',
        underline: !!properties.u && properties.u !== 'none',
        color: defaultColor,
        fontFace: DEFAULT_FONT_FACE,
        fontSize: properties.sz ? Number(properties.sz) / 100 : 16,
        breakLine: runIndex === paragraphRuns.length - 1,
      })
    })
  }

  if (!runs.length && textBody?.plainText) {
    runs.push({
      text: textBody.plainText,
      bold: false,
      italic: false,
      underline: false,
      color: defaultColor,
      fontFace: DEFAULT_FONT_FACE,
      fontSize: 16,
    })
  }

  return runs
}

function paragraphText(textBody: ExtractedTextBody | undefined) {
  const lines = (textBody?.paragraphs ?? [])
    .map((paragraph) => (paragraph.runs ?? []).map((run) => run.text ?? '').join(''))

  while (lines.length && !lines[0].trim()) {
    lines.shift()
  }
  while (lines.length && !lines[lines.length - 1]?.trim()) {
    lines.pop()
  }

  if (lines.length) {
    return lines.join('\n')
  }

  return textBody?.plainText?.trim() ?? ''
}

function extractThemeColors(supportParts?: Record<string, ExtractedSupportPart>) {
  const rawXml = supportParts?.['ppt/theme/theme1.xml']?.rawXml
  if (!rawXml) {
    return DEFAULT_THEME
  }

  const theme = { ...DEFAULT_THEME }
  for (const key of Object.keys(DEFAULT_THEME)) {
    const match = rawXml.match(
      new RegExp(
        `<a:${key}>[\\s\\S]*?(?:<a:srgbClr val="([0-9A-Fa-f]{6})"\\/?>(?:[\\s\\S]*?)<\\/a:srgbClr>|<a:srgbClr val="([0-9A-Fa-f]{6})"\\s*\\/?>|<a:sysClr[^>]*lastClr="([0-9A-Fa-f]{6})"\\s*\\/?>)[\\s\\S]*?<\\/a:${key}>`,
      ),
    )
    const color = match?.[1] || match?.[2] || match?.[3]
    if (color) {
      theme[key] = color.toUpperCase()
    }
  }
  return theme
}

function deriveTitleFromExtractedSlide(input: ExtractedSlideSpec) {
  const titleCandidate = input.shapeTree?.elements
    ?.filter((element) => element.kind === 'shape' && !!paragraphText(element.text))
    .sort((left, right) => coerceNumber(left.transform?.yPx, 0) - coerceNumber(right.transform?.yPx, 0))[0]

  const title = paragraphText(titleCandidate?.text).replace(/\s+/g, ' ').trim()
  return title || `Slide ${input.slideNumber ?? 1}`
}
