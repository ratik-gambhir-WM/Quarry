import { getConnectorAwareElementOrder } from '../shared/PowerpointLayering'
import { normalizePresentationSpec } from '../shared/PowerpointNormalizer'
import type {
  JsonObject,
  JsonValue,
  NormalizedElement,
  NormalizedPresentation,
  NormalizedSlide,
  ValidationIssue,
} from '../shared/PowerpointTypes'

export type SlideElementRef = {
  element: NormalizedElement
  key: string
  slideIndex: number
  sourcePath: string
}

export interface SlideCanvasModel {
  elementRefs: SlideElementRef[]
  issues: ValidationIssue[]
  presentation?: NormalizedPresentation
  slide?: NormalizedSlide
}

export interface BuildSlideCanvasModelOptions {
  resolveImageSource?: (source: string) => string
  slideIndex?: number
}

export function buildSlideCanvasModel(
  input: JsonValue,
  options: BuildSlideCanvasModelOptions = {},
): SlideCanvasModel {
  const slideIndex = options.slideIndex ?? 0
  const normalizedInput = options.resolveImageSource
    ? mapImageSources(input, options.resolveImageSource)
    : input
  const { presentation, issues } = normalizePresentationSpec(normalizedInput)
  const slide = presentation?.slides[slideIndex]

  if (!slide) {
    return {
      elementRefs: [],
      issues,
      presentation,
    }
  }

  const keyCounts = new Map<string, number>()
  const orderedElements = slide.preserveElementOrder
    ? slide.elements
    : getConnectorAwareElementOrder(slide)
  const elementRefs = orderedElements.map((element) => {
    const baseKey = `${slideIndex}:${element.sourcePath}`
    const occurrence = keyCounts.get(baseKey) ?? 0
    keyCounts.set(baseKey, occurrence + 1)

    return {
      element,
      key: occurrence === 0 ? baseKey : `${baseKey}:${occurrence}`,
      slideIndex,
      sourcePath: element.sourcePath,
    }
  })

  return {
    elementRefs,
    issues,
    presentation,
    slide,
  }
}

function mapImageSources(
  input: JsonValue,
  resolveImageSource: (source: string) => string,
): JsonValue {
  if (Array.isArray(input)) {
    return input.map((value) => mapImageSources(value, resolveImageSource))
  }

  if (!isRecord(input)) {
    return input
  }

  const type = String(input.type ?? input.kind ?? '').toLowerCase()
  const isImage = type === 'image' || type === 'picture'
  return Object.fromEntries(
    Object.entries(input).map(([key, value]) => [
      key,
      isImage && key === 'src' && typeof value === 'string'
        ? resolveImageSource(value)
        : value === undefined
          ? undefined
          : mapImageSources(value, resolveImageSource),
    ]),
  )
}

function isRecord(value: JsonValue): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
