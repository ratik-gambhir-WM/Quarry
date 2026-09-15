import { DEFAULT_FONT_FACE } from '../shared/PowerpointConstants'
import type {
  HorizontalAlign,
  NormalizedShapeElement,
  NormalizedTextElement,
  NormalizedTextRun,
} from '../shared/PowerpointTypes'
import { normalizeTextRuns, textFromNormalizedRuns } from '../canvas/textRuns'
import type { PlateParagraph, PlateTextLeaf, PlateValue } from './types'

type EditableTextElement = NormalizedShapeElement | NormalizedTextElement

export type RichTextEdit = {
  align: HorizontalAlign
  runs: NormalizedTextRun[]
  text: string
}

export type FromPlateValueOptions = {
  strict?: boolean
}

export function toPlateValue(element: EditableTextElement): PlateValue {
  const runs = element.kind === 'shape' ? element.textRuns : element.runs
  const fallback = getDefaultTextRun(element)
  return plateValueFromRuns(runs.length ? runs : [{ ...fallback, text: '' }], element.align)
}

export function sanitizePlateValue(
  value: unknown,
  defaults: EditableTextElement,
): PlateValue {
  const edit = fromPlateValue(value, defaults)
  return plateValueFromRuns(edit.runs, edit.align)
}

function plateValueFromRuns(
  runs: readonly NormalizedTextRun[],
  align: HorizontalAlign,
): PlateValue {
  const paragraphs: PlateParagraph[] = []
  let leaves: PlateTextLeaf[] = []

  const finishParagraph = () => {
    paragraphs.push({
      align,
      children: leaves.length ? leaves : [{ text: '' }],
      type: 'p',
    })
    leaves = []
  }

  runs.forEach((run) => {
    const parts = run.text.replace(/\r\n?/gu, '\n').split('\n')
    parts.forEach((part, index) => {
      leaves.push(toPlateLeaf({ ...run, text: part }))
      if (index < parts.length - 1) finishParagraph()
    })
    if (run.breakLine) finishParagraph()
  })

  if (leaves.length || paragraphs.length === 0) finishParagraph()
  return paragraphs
}

export function fromPlateValue(
  value: unknown,
  defaults: EditableTextElement,
  options: FromPlateValueOptions = {},
): RichTextEdit {
  const fallback = getDefaultTextRun(defaults)
  const rawNodes = Array.isArray(value) ? value : []
  if (options.strict && !Array.isArray(value)) {
    throw new TypeError('Plate value must be an array of paragraphs.')
  }

  const paragraphs = rawNodes.length ? rawNodes : [{ children: [{ text: '' }], type: 'p' }]
  const runs: NormalizedTextRun[] = []
  let align: HorizontalAlign = defaults.align

  paragraphs.forEach((node, paragraphIndex) => {
    const paragraph = asRecord(node)
    if (options.strict && paragraph?.type !== 'p') {
      throw new TypeError(`Unsupported Plate node at index ${paragraphIndex}.`)
    }
    if (paragraphIndex === 0 && isHorizontalAlign(paragraph?.align)) {
      align = paragraph.align
    }

    const leaves = collectLeaves(paragraph?.children, options.strict ?? false)
    const sourceLeaves = leaves.length ? leaves : [{ text: '' }]
    sourceLeaves.forEach((leaf) => {
      runs.push({
        bold: asBoolean(leaf.bold) ?? fallback.bold,
        color: normalizeColor(leaf.color, fallback.color),
        fontFace: asNonEmptyString(leaf.fontFamily) ?? fallback.fontFace,
        fontSize: normalizeFontSize(leaf.fontSize, fallback.fontSize),
        italic: asBoolean(leaf.italic) ?? fallback.italic,
        text: leaf.text,
        underline: asBoolean(leaf.underline) ?? fallback.underline,
      })
    })
    if (paragraphIndex < paragraphs.length - 1) {
      runs[runs.length - 1].breakLine = true
    }
  })

  const normalizedRuns = normalizeTextRuns(runs, fallback)
  return {
    align,
    runs: normalizedRuns,
    text: textFromNormalizedRuns(normalizedRuns),
  }
}

export function getDefaultTextRun(element: EditableTextElement): NormalizedTextRun {
  const runs = element.kind === 'shape' ? element.textRuns : element.runs
  const existing = runs[0]
  return {
    bold: existing?.bold ?? element.bold,
    color:
      existing?.color ?? (element.kind === 'shape' ? element.textColor : element.color),
    fontFace: existing?.fontFace || element.fontFace || DEFAULT_FONT_FACE,
    fontSize: normalizeFontSize(existing?.fontSize, element.fontSize),
    italic: existing?.italic ?? (element.kind === 'text' ? element.italic : false),
    text: '',
    underline: existing?.underline ?? false,
  }
}

function toPlateLeaf(run: NormalizedTextRun): PlateTextLeaf {
  return {
    bold: run.bold,
    color: `#${normalizeColor(run.color, '111827')}`,
    fontFamily: run.fontFace,
    fontSize: `${normalizeFontSize(run.fontSize, 16)}px`,
    italic: run.italic,
    text: run.text,
    underline: run.underline,
  }
}

function collectLeaves(value: unknown, strict: boolean): PlateTextLeaf[] {
  if (!Array.isArray(value)) return []
  const leaves: PlateTextLeaf[] = []
  value.forEach((node) => {
    const record = asRecord(node)
    if (!record) return
    if (typeof record.text === 'string') {
      if (strict) assertSupportedLeaf(record)
      leaves.push({
        ...(typeof record.bold === 'boolean' ? { bold: record.bold } : undefined),
        ...(typeof record.color === 'string' ? { color: record.color } : undefined),
        ...(typeof record.fontFamily === 'string'
          ? { fontFamily: record.fontFamily }
          : undefined),
        ...(typeof record.fontSize === 'string' || typeof record.fontSize === 'number'
          ? { fontSize: String(record.fontSize) }
          : undefined),
        ...(typeof record.italic === 'boolean' ? { italic: record.italic } : undefined),
        text: record.text,
        ...(typeof record.underline === 'boolean'
          ? { underline: record.underline }
          : undefined),
      })
      return
    }
    if (strict) throw new TypeError('Unsupported nested Plate node.')
    leaves.push(...collectLeaves(record.children, false))
  })
  return leaves
}

function assertSupportedLeaf(leaf: Record<string, unknown>) {
  const supported = new Set([
    'bold',
    'color',
    'fontFamily',
    'fontSize',
    'italic',
    'text',
    'underline',
  ])
  const unsupported = Object.keys(leaf).find((key) => !supported.has(key))
  if (unsupported) throw new TypeError(`Unsupported Plate mark: ${unsupported}.`)
}

function normalizeColor(value: unknown, fallback: string) {
  const stringValue = typeof value === 'string' ? value.trim() : ''
  const hex = stringValue.replace(/^#/u, '')
  if (/^[0-9a-f]{6}$/iu.test(hex)) return hex.toUpperCase()
  if (/^[0-9a-f]{3}$/iu.test(hex)) {
    return hex.split('').map((character) => character.repeat(2)).join('').toUpperCase()
  }
  const rgb = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/iu.exec(stringValue)
  if (rgb) {
    return rgb.slice(1, 4).map((component) =>
      Math.min(255, Number(component)).toString(16).padStart(2, '0'),
    ).join('').toUpperCase()
  }
  return fallback.replace(/^#/u, '').toUpperCase()
}

function normalizeFontSize(value: unknown, fallback: number) {
  const numeric = typeof value === 'number' ? value : Number.parseFloat(String(value ?? ''))
  if (!Number.isFinite(numeric)) return Math.min(200, Math.max(5, fallback))
  return Math.min(200, Math.max(5, numeric))
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined
}

function asBoolean(value: unknown) {
  return typeof value === 'boolean' ? value : undefined
}

function asNonEmptyString(value: unknown) {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined
}

function isHorizontalAlign(value: unknown): value is HorizontalAlign {
  return value === 'left' || value === 'center' || value === 'right'
}
