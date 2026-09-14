export interface TextEditorOptions {
  /** Six-digit hex colors, with or without a leading #. */
  colors?: readonly string[]
  fontFamilies?: readonly string[]
  /** Font sizes in PowerPoint points. */
  fontSizes?: readonly number[]
}

export type ResolvedTextEditorOptions = {
  colors: string[]
  fontFamilies: string[]
  fontSizes: number[]
}

const DEFAULT_TEXT_EDITOR_OPTIONS: ResolvedTextEditorOptions = {
  colors: ['111827', 'FFFFFF', '2563EB', 'DC2626', '16A34A', 'D97706'],
  fontFamilies: ['Aptos', 'Arial', 'Calibri', 'Georgia', 'Times New Roman'],
  fontSizes: [8, 10, 12, 14, 16, 18, 24, 32, 40, 48, 60, 72],
}

export function resolveTextEditorOptions(
  options?: TextEditorOptions,
): ResolvedTextEditorOptions {
  return {
    colors: normalizeColors(options?.colors) ?? [...DEFAULT_TEXT_EDITOR_OPTIONS.colors],
    fontFamilies:
      normalizeFontFamilies(options?.fontFamilies) ??
      [...DEFAULT_TEXT_EDITOR_OPTIONS.fontFamilies],
    fontSizes:
      normalizeFontSizes(options?.fontSizes) ?? [...DEFAULT_TEXT_EDITOR_OPTIONS.fontSizes],
  }
}

function normalizeColors(values?: readonly string[]) {
  if (!values) return undefined
  const colors = Array.from(
    new Set(values.map((value) => value.replace(/^#/u, '').toUpperCase()).filter(isHexColor)),
  )
  return colors.length ? colors : undefined
}

function normalizeFontFamilies(values?: readonly string[]) {
  if (!values) return undefined
  const families = Array.from(new Set(values.map((value) => value.trim()).filter(Boolean)))
  return families.length ? families : undefined
}

function normalizeFontSizes(values?: readonly number[]) {
  if (!values) return undefined
  const sizes = Array.from(
    new Set(
      values
        .filter((value) => Number.isFinite(value))
        .map((value) => Math.min(200, Math.max(5, value))),
    ),
  ).sort((left, right) => left - right)
  return sizes.length ? sizes : undefined
}

function isHexColor(value: string) {
  return /^[0-9A-F]{6}$/u.test(value)
}
