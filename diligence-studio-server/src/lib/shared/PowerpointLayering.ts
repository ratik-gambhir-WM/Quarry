import type {
  NormalizedElement,
  NormalizedLineElement,
  NormalizedSlide,
  SlideForElementLayering,
} from './PowerpointTypes'
import { MIN_ELEMENT_SIZE_PX } from './PowerpointConstants'

const LARGE_REGION_AREA_RATIO = 0.08
const TALL_LANE_HEIGHT_RATIO = 0.45
const TALL_LANE_WIDTH_RATIO = 0.12
const BRANDED_BACKGROUND_LAYER = 0
const SLIDE_CONTAINER_LAYER = 10
const CONNECTOR_LAYER = 20
const CONTENT_LAYER = 30

export function getConnectorAwareElementOrder(slide: SlideForElementLayering) {
  return slide.elements
    .map((element, index) => ({
      element,
      index,
      layer: getConnectorAwareLayer(element, slide),
    }))
    .sort((a, b) => a.layer - b.layer || a.index - b.index)
    .map(({ element }) => element)
}

function getConnectorAwareLayer(element: NormalizedElement, slide: SlideForElementLayering) {
  if (isBrandedBackgroundDecoration(element)) {
    return BRANDED_BACKGROUND_LAYER
  }

  if (isSlideContainerElement(element, slide)) {
    return SLIDE_CONTAINER_LAYER
  }

  if (element.kind === 'line') {
    return CONNECTOR_LAYER
  }

  return CONTENT_LAYER
}

function isBrandedBackgroundDecoration(element: NormalizedElement) {
  return (
    element.id === 'west-monroe-footer' ||
    element.id.startsWith('west-monroe-dot-')
  )
}

function isSlideContainerElement(element: NormalizedElement, slide: SlideForElementLayering) {
  if (element.kind !== 'shape' || element.shape !== 'rect') {
    return false
  }

  const slideArea = Math.max(slide.width * slide.height, MIN_ELEMENT_SIZE_PX)
  const elementArea = Math.max(element.w * element.h, 0)
  const coversLargeRegion = elementArea / slideArea >= LARGE_REGION_AREA_RATIO
  const coversTallLane =
    element.h / slide.height >= TALL_LANE_HEIGHT_RATIO &&
    element.w / slide.width >= TALL_LANE_WIDTH_RATIO

  return coversLargeRegion || coversTallLane
}

export function addConnectorOcclusionRects(
  elements: NormalizedElement[],
  slide: Pick<NormalizedSlide, 'height' | 'width'>,
): NormalizedElement[] {
  const slideForLayering = { ...slide, elements }
  const occlusionRects = elements
    .filter((element): element is Exclude<NormalizedElement, NormalizedLineElement> =>
      element.kind !== 'line' && !isSlideContainerElement(element, slideForLayering),
    )
    .map(({ h, w, x, y }) => ({ h, w, x, y }))

  return elements.map((element) => {
    if (element.kind !== 'line') {
      return element
    }

    return {
      ...element,
      occlusionRects: element.endArrow === 'none' ? occlusionRects : [],
    }
  })
}
