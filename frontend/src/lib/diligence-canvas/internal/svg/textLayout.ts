import type {
  HorizontalAlign,
  NormalizedTextRun,
  VerticalAlign,
} from '../shared/PowerpointTypes'
import { SLIDE_TEXT_LINE_HEIGHT } from '../shared/PowerpointConstants'

export type TextMeasure = (
  text: string,
  run: NormalizedTextRun,
  fontSize: number,
) => number

export type SvgTextSegment = {
  run: NormalizedTextRun
  text: string
  width: number
  x: number
}

export type SvgTextLine = {
  baseline: number
  height: number
  segments: SvgTextSegment[]
  width: number
}

export type SvgTextLayout = {
  fontScale: number
  height: number
  lines: SvgTextLine[]
}

export type LayoutSvgTextOptions = {
  align: HorizontalAlign
  fallbackRun: NormalizedTextRun
  fallbackText: string
  fixedFontScale?: number
  height: number
  measure: TextMeasure
  padding: number
  runs: NormalizedTextRun[]
  valign: VerticalAlign
  width: number
}

type TextAtom =
  | { kind: 'break' }
  | { kind: 'text'; run: NormalizedTextRun; text: string }

type UnpositionedLine = Omit<SvgTextLine, 'baseline'>

const MIN_FONT_SIZE = 5

export function layoutSvgText(options: LayoutSvgTextOptions): SvgTextLayout {
  const availableHeight = Math.max(options.height - options.padding * 2, 1)
  const availableWidth = Math.max(options.width - options.padding * 2, 1)
  const atoms = buildTextAtoms(options)
  const largestFontSize = Math.max(
    options.fallbackRun.fontSize,
    ...options.runs.map((run) => run.fontSize),
  )
  const minScale = Math.min(1, MIN_FONT_SIZE / Math.max(largestFontSize, MIN_FONT_SIZE))
  const maxFontScale = Math.min(Math.max(options.fixedFontScale ?? 1, minScale), 1)

  let fontScale = maxFontScale
  let lines = wrapText(atoms, availableWidth, fontScale, options.measure, options.fallbackRun)
  let textHeight = getTextHeight(lines)

  if (options.fixedFontScale === undefined && textHeight > availableHeight) {
    let low = minScale
    let high = maxFontScale
    let bestLines = wrapText(atoms, availableWidth, low, options.measure, options.fallbackRun)
    let bestScale = low

    for (let attempt = 0; attempt < 14; attempt += 1) {
      const candidateScale = (low + high) / 2
      const candidateLines = wrapText(
        atoms,
        availableWidth,
        candidateScale,
        options.measure,
        options.fallbackRun,
      )

      if (getTextHeight(candidateLines) <= availableHeight) {
        bestScale = candidateScale
        bestLines = candidateLines
        low = candidateScale
      } else {
        high = candidateScale
      }
    }

    fontScale = bestScale
    lines = bestLines
    textHeight = getTextHeight(lines)
  }

  const verticalOffset = getVerticalOffset(options.valign, availableHeight, textHeight)
  let top = options.padding + verticalOffset

  return {
    fontScale,
    height: textHeight,
    lines: lines.map((line) => {
      const x = getHorizontalOffset(options.align, options.padding, availableWidth, line.width)
      let segmentX = x
      const positionedLine: SvgTextLine = {
        ...line,
        baseline: top + line.height * 0.82,
        segments: line.segments.map((segment) => {
          const positionedSegment = { ...segment, x: segmentX }
          segmentX += segment.width
          return positionedSegment
        }),
      }
      top += line.height
      return positionedLine
    }),
  }
}

export function createCanvasTextMeasurer(): TextMeasure {
  let context: CanvasRenderingContext2D | null | undefined
  const cache = new Map<string, number>()

  return (text, run, fontSize) => {
    const key = `${run.fontFace}|${fontSize.toFixed(3)}|${run.bold ? 1 : 0}|${run.italic ? 1 : 0}|${text}`
    const cached = cache.get(key)
    if (cached !== undefined) {
      return cached
    }

    if (context === undefined) {
      context =
        typeof document === 'undefined'
          ? null
          : document.createElement('canvas').getContext('2d')
    }

    if (context) {
      context.font = `${run.italic ? 'italic ' : ''}${run.bold ? '700 ' : '400 '}${fontSize}px ${quoteFontFace(run.fontFace)}`
    }

    const width = context?.measureText(text).width ?? text.length * fontSize * 0.52
    if (cache.size > 10_000) {
      cache.clear()
    }
    cache.set(key, width)
    return width
  }
}

function buildTextAtoms(options: LayoutSvgTextOptions): TextAtom[] {
  const sourceRuns = options.runs.length
    ? options.runs
    : [{ ...options.fallbackRun, text: options.fallbackText }]
  const atoms: TextAtom[] = []

  sourceRuns.forEach((run) => {
    const parts = run.text.split('\n')
    parts.forEach((part, partIndex) => {
      const tokens = part.match(/[^\S\n]+|[^\s]+/g) ?? []
      tokens.forEach((text) => atoms.push({ kind: 'text', run, text }))
      if (partIndex < parts.length - 1) {
        atoms.push({ kind: 'break' })
      }
    })

    if (run.breakLine && !run.text.endsWith('\n')) {
      atoms.push({ kind: 'break' })
    }
  })

  if (atoms[atoms.length - 1]?.kind === 'break') {
    atoms.pop()
  }

  return atoms
}

function wrapText(
  atoms: TextAtom[],
  availableWidth: number,
  fontScale: number,
  measure: TextMeasure,
  fallbackRun: NormalizedTextRun,
) {
  const lines: UnpositionedLine[] = []
  let segments: SvgTextSegment[] = []
  let width = 0
  let lineFontSize = fallbackRun.fontSize * fontScale

  const pushLine = () => {
    lines.push({
      height: Math.max(
        lineFontSize * SLIDE_TEXT_LINE_HEIGHT,
        MIN_FONT_SIZE * SLIDE_TEXT_LINE_HEIGHT,
      ),
      segments,
      width,
    })
    segments = []
    width = 0
    lineFontSize = fallbackRun.fontSize * fontScale
  }

  const append = (text: string, run: NormalizedTextRun) => {
    if (!text) {
      return
    }
    const fontSize = Math.max(run.fontSize * fontScale, MIN_FONT_SIZE)
    const segmentWidth = measure(text, run, fontSize)
    const previous = segments[segments.length - 1]
    if (previous?.run === run) {
      previous.text += text
      previous.width += segmentWidth
    } else {
      segments.push({ run, text, width: segmentWidth, x: 0 })
    }
    width += segmentWidth
    lineFontSize = Math.max(lineFontSize, fontSize)
  }

  atoms.forEach((atom) => {
    if (atom.kind === 'break') {
      pushLine()
      return
    }

    const isWhitespace = /^\s+$/.test(atom.text)
    const fontSize = Math.max(atom.run.fontSize * fontScale, MIN_FONT_SIZE)
    const atomWidth = measure(atom.text, atom.run, fontSize)

    if (isWhitespace && segments.length === 0) {
      return
    }

    if (width + atomWidth <= availableWidth) {
      append(atom.text, atom.run)
      return
    }

    if (segments.length > 0) {
      pushLine()
    }

    if (isWhitespace) {
      return
    }

    if (atomWidth <= availableWidth) {
      append(atom.text, atom.run)
      return
    }

    for (const character of atom.text) {
      const characterWidth = measure(character, atom.run, fontSize)
      if (segments.length > 0 && width + characterWidth > availableWidth) {
        pushLine()
      }
      append(character, atom.run)
    }
  })

  if (segments.length > 0 || lines.length === 0) {
    pushLine()
  }

  return lines
}

function getTextHeight(lines: UnpositionedLine[]) {
  return lines.reduce((height, line) => height + line.height, 0)
}

function getVerticalOffset(valign: VerticalAlign, availableHeight: number, textHeight: number) {
  if (valign === 'bottom') {
    return Math.max(availableHeight - textHeight, 0)
  }
  if (valign === 'middle') {
    return Math.max((availableHeight - textHeight) / 2, 0)
  }
  return 0
}

function getHorizontalOffset(
  align: HorizontalAlign,
  padding: number,
  availableWidth: number,
  lineWidth: number,
) {
  if (align === 'right') {
    return padding + Math.max(availableWidth - lineWidth, 0)
  }
  if (align === 'center') {
    return padding + Math.max((availableWidth - lineWidth) / 2, 0)
  }
  return padding
}

function quoteFontFace(fontFace: string) {
  return fontFace.includes(' ') ? `"${fontFace.replace(/"/g, '')}"` : fontFace
}
