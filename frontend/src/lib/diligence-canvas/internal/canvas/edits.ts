import type {
  JsonObject,
  JsonValue,
  HorizontalAlign,
  NormalizedElement,
  NormalizedLineElement,
  NormalizedTextRun,
} from '../shared/PowerpointTypes'
import type { SlideElementRef } from './model'
import {
  buildNormalizedTextRuns,
  buildRawTextRuns,
  normalizeTextRuns,
  textFromNormalizedRuns,
} from './textRuns'

export type ElementEdit = {
  align?: HorizontalAlign
  h?: number
  lineType?: NormalizedLineElement['lineType']
  runs?: NormalizedTextRun[]
  text?: string
  w?: number
  x?: number
  x1?: number
  x2?: number
  y?: number
  y1?: number
  y2?: number
}

export type ElementMutationLocator = Pick<SlideElementRef, 'slideIndex' | 'sourcePath'> & {
  element: Pick<NormalizedElement, 'id' | 'kind'>
}

export type ElementEditRequest = {
  edit: ElementEdit
  locator: ElementMutationLocator
}

export function applyElementEditToInput<TInput extends JsonValue>(
  input: TInput,
  locator: ElementMutationLocator,
  edit: ElementEdit,
): TInput {
  return applyElementEditsToInput(input, [{ edit, locator }])
}

export function applyElementEditsToInput<TInput extends JsonValue>(
  input: TInput,
  requests: ElementEditRequest[],
): TInput {
  const nextInput = cloneJsonValue(input)
  requests.forEach(({ edit, locator }) => {
    const target = locateRawElement(nextInput, locator)
    if (target) {
      applyRawElementEdit(target, edit)
    }
  })

  return nextInput
}

export function deleteElementsFromInput<TInput extends JsonValue>(
  input: TInput,
  locators: ElementMutationLocator[],
): TInput {
  const nextInput = cloneJsonValue(input)
  const targets = new Set(
    locators
      .map((locator) => locateRawElement(nextInput, locator))
      .filter((target): target is JsonObject => target !== undefined),
  )

  if (targets.size > 0) {
    deleteRawElementObjects(nextInput, targets)
  }

  return nextInput
}

export function applyElementEdit(element: NormalizedElement, edit: ElementEdit): NormalizedElement {
  if (element.kind === 'line') {
    return {
      ...element,
      lineType: edit.lineType ?? element.lineType,
      x1: edit.x1 ?? element.x1,
      y1: edit.y1 ?? element.y1,
      x2: edit.x2 ?? element.x2,
      y2: edit.y2 ?? element.y2,
    }
  }

  const geometryEdit = {
    ...(edit.h !== undefined ? { h: edit.h } : undefined),
    ...(edit.w !== undefined ? { w: edit.w } : undefined),
    ...(edit.x !== undefined ? { x: edit.x } : undefined),
    ...(edit.y !== undefined ? { y: edit.y } : undefined),
  }

  if (element.kind === 'image') {
    return { ...element, ...geometryEdit }
  }

  if (element.kind === 'text') {
    const runs = edit.runs
      ? normalizeTextRuns(edit.runs, element.runs[0])
      : undefined
    const text = edit.text ?? (runs ? textFromNormalizedRuns(runs) : element.text)
    return {
      ...element,
      ...geometryEdit,
      align: edit.align ?? element.align,
      ...(runs
        ? { runs, text }
        : edit.text !== undefined
          ? { runs: buildNormalizedTextRuns(element, text), text }
        : undefined),
    }
  }

  const runs = edit.runs
    ? normalizeTextRuns(edit.runs, element.textRuns[0])
    : undefined
  const text = edit.text ?? (runs ? textFromNormalizedRuns(runs) : element.label)
  return {
    ...element,
    ...geometryEdit,
    align: edit.align ?? element.align,
    ...(runs
      ? { label: text, textRuns: runs }
      : edit.text !== undefined
        ? { label: text, textRuns: buildNormalizedTextRuns(element, text) }
      : undefined),
  }
}

function locateRawElement(
  input: JsonValue,
  locator: ElementMutationLocator,
): JsonObject | undefined {
  const elementIndex = getElementIndex(locator.sourcePath)
  const slide = getRawSlides(input)[locator.slideIndex]
  const sourceCandidate =
    elementIndex !== undefined && Array.isArray(slide?.elements)
      ? slide.elements[elementIndex]
      : undefined

  if (isRecord(sourceCandidate) && rawElementMatches(sourceCandidate, locator)) {
    return sourceCandidate
  }

  const matches: JsonObject[] = []
  collectMatchingRawElements(input, locator, matches)
  return matches.length === 1 ? matches[0] : undefined
}

function getRawSlides(input: JsonValue): JsonObject[] {
  if (Array.isArray(input)) {
    return input.filter(isRecord)
  }

  if (!isRecord(input)) {
    return []
  }

  if (Array.isArray(input.elements)) {
    return [input]
  }

  const presentation = isRecord(input.presentation) ? input.presentation : input
  if (Array.isArray(presentation.slides)) {
    return presentation.slides.filter(isRecord)
  }

  if (isRecord(presentation.slide)) {
    return [presentation.slide]
  }

  if (isRecord(input.slide)) {
    return [input.slide]
  }

  return []
}

function getElementIndex(sourcePath: string) {
  const match = sourcePath.match(/(?:^|\.)elements\[(\d+)](?:$|\.)/)
  if (!match) {
    return undefined
  }

  const value = Number(match[1])
  return Number.isInteger(value) ? value : undefined
}

function rawElementMatches(
  rawElement: JsonObject,
  locator: ElementMutationLocator,
) {
  return getRawKind(rawElement) === locator.element.kind && getRawIds(rawElement).has(locator.element.id)
}

function getRawIds(rawElement: JsonObject) {
  const ids = new Set<string>()
  if (typeof rawElement.id === 'string') {
    ids.add(rawElement.id)
  }

  if (isRecord(rawElement.nonVisual)) {
    const nonVisualId = rawElement.nonVisual.id
    if (typeof nonVisualId === 'number' || typeof nonVisualId === 'string') {
      ids.add(`element-${nonVisualId}`)
    }
  }

  return ids
}

function getRawKind(rawElement: JsonObject): NormalizedElement['kind'] | undefined {
  const rawKind = String(
    rawElement.kind ?? rawElement.type ?? rawElement.elementType ?? rawElement.shape ?? '',
  ).toLowerCase()

  if (rawKind === 'text' || rawKind === 'textbox') {
    return 'text'
  }
  if (rawKind === 'image' || rawKind === 'picture' || rawKind === 'pic') {
    return 'image'
  }
  if (rawKind === 'line' || rawKind === 'connector') {
    return 'line'
  }
  if (rawKind === 'shape' || rawElement.shape !== undefined) {
    return 'shape'
  }

  return undefined
}

function collectMatchingRawElements(
  value: JsonValue | undefined,
  locator: ElementMutationLocator,
  matches: JsonObject[],
) {
  if (Array.isArray(value)) {
    value.forEach((item) => collectMatchingRawElements(item, locator, matches))
    return
  }

  if (!isRecord(value)) {
    return
  }

  if (rawElementMatches(value, locator)) {
    matches.push(value)
  }

  Object.values(value).forEach((item) => collectMatchingRawElements(item, locator, matches))
}

function deleteRawElementObjects(value: JsonValue | undefined, targets: Set<JsonObject>): boolean {
  if (Array.isArray(value)) {
    let didDelete = false
    for (let index = value.length - 1; index >= 0; index -= 1) {
      const item = value[index]
      if (isRecord(item) && targets.has(item)) {
        value.splice(index, 1)
        didDelete = true
      } else {
        didDelete = deleteRawElementObjects(item, targets) || didDelete
      }
    }
    return didDelete
  }

  if (!isRecord(value)) {
    return false
  }

  return Object.values(value).some((item) => deleteRawElementObjects(item, targets))
}

function applyRawElementEdit(element: JsonObject, edit: ElementEdit) {
  for (const key of ['x1', 'y1', 'x2', 'y2', 'lineType'] as const) {
    if (edit[key] !== undefined) {
      element[key] = edit[key]
    }
  }

  for (const [key, alias] of [
    ['x', 'left'],
    ['y', 'top'],
    ['w', 'width'],
    ['h', 'height'],
  ] as const) {
    if (edit[key] !== undefined) {
      element[key] = edit[key]
      if (alias in element) {
        element[alias] = edit[key]
      }
    }
  }

  if (edit.align !== undefined) {
    element.align = edit.align
  }

  if (edit.runs !== undefined) {
    const runs = normalizeTextRuns(edit.runs)
    const text = edit.text ?? textFromNormalizedRuns(runs)
    element.text = text
    if ('label' in element) {
      element.label = text
    }
    element.runs = runs.map((run) => ({ ...run }))
  } else if (edit.text !== undefined) {
    element.text = edit.text
    if ('label' in element) {
      element.label = edit.text
    }
    element.runs = buildRawTextRuns(element, edit.text)
  }
}

function cloneJsonValue<T extends JsonValue>(value: T): T {
  if (typeof structuredClone === 'function') {
    return structuredClone(value)
  }

  if (Array.isArray(value)) {
    return value.map((item) => cloneJsonValue(item)) as T
  }

  if (isRecord(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [
        key,
        entry === undefined ? undefined : cloneJsonValue(entry),
      ]),
    ) as T
  }

  return value
}

function isRecord(value: JsonValue | undefined): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
