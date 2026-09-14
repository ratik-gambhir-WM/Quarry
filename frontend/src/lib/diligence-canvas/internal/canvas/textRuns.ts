import { DEFAULT_FONT_FACE } from '../shared/PowerpointConstants'
import type {
  NormalizedShapeElement,
  NormalizedTextElement,
  NormalizedTextRun,
  JsonObject,
  JsonValue,
} from '../shared/PowerpointTypes'

export function normalizeTextRuns(
  runs: readonly NormalizedTextRun[],
  fallback?: NormalizedTextRun,
): NormalizedTextRun[] {
  const normalized: NormalizedTextRun[] = []

  runs.forEach((run) => {
    const next: NormalizedTextRun = {
      bold: typeof run.bold === 'boolean' ? run.bold : (fallback?.bold ?? false),
      color: normalizeHexColor(run.color, fallback?.color ?? '111827'),
      fontFace: run.fontFace?.trim() || fallback?.fontFace || DEFAULT_FONT_FACE,
      fontSize: normalizePointSize(run.fontSize, fallback?.fontSize ?? 16),
      italic: typeof run.italic === 'boolean' ? run.italic : (fallback?.italic ?? false),
      text: String(run.text ?? '').replace(/\r\n?/gu, '\n'),
      underline:
        typeof run.underline === 'boolean' ? run.underline : (fallback?.underline ?? false),
      ...(run.breakLine ? { breakLine: true } : undefined),
    }
    const previous = normalized[normalized.length - 1]
    if (previous && !previous.breakLine && sameTextRunStyle(previous, next)) {
      previous.text += next.text
      if (next.breakLine) previous.breakLine = true
    } else {
      normalized.push(next)
    }
  })

  return normalized.length ? normalized : [{
    bold: fallback?.bold ?? false,
    color: normalizeHexColor(fallback?.color, '111827'),
    fontFace: fallback?.fontFace || DEFAULT_FONT_FACE,
    fontSize: normalizePointSize(fallback?.fontSize, 16),
    italic: fallback?.italic ?? false,
    text: '',
    underline: fallback?.underline ?? false,
  }]
}

export function textFromNormalizedRuns(runs: readonly NormalizedTextRun[]) {
  return runs.map((run) => `${run.text}${run.breakLine ? '\n' : ''}`).join('')
}

function sameTextRunStyle(left: NormalizedTextRun, right: NormalizedTextRun) {
  return left.bold === right.bold &&
    left.color === right.color &&
    left.fontFace === right.fontFace &&
    left.fontSize === right.fontSize &&
    left.italic === right.italic &&
    left.underline === right.underline
}

function normalizeHexColor(value: string | undefined, fallback: string) {
  const candidate = String(value ?? '').replace(/^#/u, '')
  return /^[0-9a-f]{6}$/iu.test(candidate)
    ? candidate.toUpperCase()
    : fallback.replace(/^#/u, '').toUpperCase()
}

function normalizePointSize(value: number | undefined, fallback: number) {
  const candidate = Number.isFinite(value) ? Number(value) : fallback
  return Math.min(200, Math.max(5, candidate))
}

export function buildNormalizedTextRuns(
  element: NormalizedShapeElement | NormalizedTextElement,
  text: string,
): NormalizedTextRun[] {
  const existingRuns = element.kind === 'shape' ? element.textRuns : element.runs
  const fallbackRun = existingRuns[0]
  const lines = text.split('\n')

  return lines.map((line, index) => ({
    bold: existingRuns[index]?.bold ?? fallbackRun?.bold ?? element.bold,
    breakLine: index < lines.length - 1,
    color:
      existingRuns[index]?.color ??
      fallbackRun?.color ??
      ('color' in element ? element.color : element.textColor),
    fontFace: existingRuns[index]?.fontFace ?? fallbackRun?.fontFace ?? DEFAULT_FONT_FACE,
    fontSize: existingRuns[index]?.fontSize ?? fallbackRun?.fontSize ?? element.fontSize,
    italic:
      existingRuns[index]?.italic ??
      fallbackRun?.italic ??
      ('italic' in element ? element.italic : false),
    text: line,
    underline: existingRuns[index]?.underline ?? fallbackRun?.underline ?? false,
  }))
}

export function buildRawTextRuns(element: JsonObject, text: string) {
  const existingRuns = Array.isArray(element.runs) ? element.runs.filter(isRecord) : []
  const fallbackRun = existingRuns[0]
  const lines = text.split('\n')

  return lines.map((line, index) => {
    const matchingRun = existingRuns[index]
    const color =
      asString(matchingRun?.color) ||
      asString(fallbackRun?.color) ||
      asString(element.textColor) ||
      asString(element.color)
    const fontSize =
      asNumber(matchingRun?.fontSize) ??
      asNumber(fallbackRun?.fontSize) ??
      asNumber(element.fontSize)
    const bold = matchingRun
      ? (asBoolean(matchingRun.bold) ?? false)
      : (asBoolean(fallbackRun?.bold) ?? asBoolean(element.bold))
    const italic = matchingRun
      ? (asBoolean(matchingRun.italic) ?? false)
      : (asBoolean(fallbackRun?.italic) ?? asBoolean(element.italic))
    const underline = matchingRun
      ? (asBoolean(matchingRun.underline) ?? false)
      : asBoolean(fallbackRun?.underline)

    return {
      ...(bold !== undefined ? { bold } : undefined),
      breakLine: index < lines.length - 1,
      ...(color ? { color } : undefined),
      fontFace:
        asString(matchingRun?.fontFace) ||
        asString(fallbackRun?.fontFace) ||
        asString(element.fontFace) ||
        DEFAULT_FONT_FACE,
      ...(fontSize !== undefined ? { fontSize } : undefined),
      ...(italic !== undefined ? { italic } : undefined),
      text: line,
      ...(underline !== undefined ? { underline } : undefined),
    }
  })
}

function isRecord(value: JsonValue | undefined): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function asString(value: JsonValue | undefined) {
  return typeof value === 'string' ? value : ''
}

function asNumber(value: JsonValue | undefined) {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

function asBoolean(value: JsonValue | undefined) {
  return typeof value === 'boolean' ? value : undefined
}
