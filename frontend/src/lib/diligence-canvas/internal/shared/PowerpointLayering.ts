import type {
  LineOcclusionRect,
  NormalizedElement,
  NormalizedLineElement,
  NormalizedSlide,
  SlideForElementLayering,
} from './PowerpointTypes'

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
    return 0
  }

  if (isSlideContainerElement(element, slide)) {
    return 10
  }

  if (element.kind === 'line') {
    return 20
  }

  return 30
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

  const slideArea = Math.max(slide.width * slide.height, 1)
  const elementArea = Math.max(element.w * element.h, 0)
  const coversLargeRegion = elementArea / slideArea >= 0.08
  const coversTallLane = element.h / slide.height >= 0.45 && element.w / slide.width >= 0.12

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
    .map(getElementBounds)

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

function getElementBounds(element: Exclude<NormalizedElement, NormalizedLineElement>): LineOcclusionRect {
  return {
    h: element.h,
    w: element.w,
    x: element.x,
    y: element.y,
  }
}
