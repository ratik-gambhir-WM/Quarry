import {
  useCallback,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
  type WheelEvent as ReactWheelEvent,
} from 'react'

const MIN_ZOOM = 0.15
const MAX_ZOOM = 4
const ZOOM_STEP = 1.25

export type SvgViewportState = {
  x: number
  y: number
  zoom: number
}

type PanGesture = {
  lastPoint: Point
  pointerId: number
}

type PinchGesture = {
  distance: number
  midpoint: Point
}

type Point = {
  x: number
  y: number
}

export function useSvgViewport({
  slideHeight,
  slideWidth,
  svgRef,
}: {
  slideHeight: number
  slideWidth: number
  svgRef: RefObject<SVGSVGElement | null>
}) {
  const dimensionsKey = `${slideWidth}:${slideHeight}`
  const [storedState, setStoredState] = useState<{
    dimensionsKey: string
    viewport: SvgViewportState
  }>(() => ({ dimensionsKey, viewport: { x: 0, y: 0, zoom: 1 } }))
  const state = storedState.dimensionsKey === dimensionsKey
    ? storedState.viewport
    : { x: 0, y: 0, zoom: 1 }
  const stateRef = useRef(state)
  const panRef = useRef<PanGesture | undefined>(undefined)
  const pinchRef = useRef<PinchGesture | undefined>(undefined)
  const touchPointsRef = useRef(new Map<number, Point>())
  stateRef.current = state

  const updateState = useCallback(
    (nextState: SvgViewportState) => {
      const clampedState = clampViewport(nextState, slideWidth, slideHeight)
      stateRef.current = clampedState
      setStoredState((currentState) =>
        currentState.dimensionsKey === dimensionsKey &&
        viewportStatesMatch(currentState.viewport, clampedState)
          ? currentState
          : { dimensionsKey, viewport: clampedState },
      )
    },
    [dimensionsKey, slideHeight, slideWidth],
  )

  const reset = useCallback(() => {
    updateState({ x: 0, y: 0, zoom: 1 })
  }, [updateState])

  const zoomAt = useCallback(
    (nextZoom: number, anchor?: Point) => {
      const currentState = stateRef.current
      const zoom = clamp(nextZoom, MIN_ZOOM, MAX_ZOOM)
      const point = anchor ?? { x: slideWidth / 2, y: slideHeight / 2 }
      const contentX = (point.x - currentState.x) / currentState.zoom
      const contentY = (point.y - currentState.y) / currentState.zoom
      updateState({
        x: point.x - contentX * zoom,
        y: point.y - contentY * zoom,
        zoom,
      })
    },
    [slideHeight, slideWidth, updateState],
  )

  const zoomIn = useCallback(() => {
    zoomAt(stateRef.current.zoom * ZOOM_STEP)
  }, [zoomAt])

  const zoomOut = useCallback(() => {
    zoomAt(stateRef.current.zoom / ZOOM_STEP)
  }, [zoomAt])

  const handleWheel = useCallback(
    (event: ReactWheelEvent<SVGSVGElement>) => {
      const svg = svgRef.current
      if (!svg) {
        return
      }

      event.preventDefault()
      if (event.ctrlKey) {
        return
      }

      const delta = clientDeltaToSvg(svg, event.deltaX, event.deltaY)
      if (!delta) {
        return
      }

      const currentState = stateRef.current
      updateState({
        ...currentState,
        x: currentState.x - delta.x,
        y: currentState.y - delta.y,
      })
    },
    [svgRef, updateState],
  )

  const handlePointerDownCapture = useCallback(
    (event: ReactPointerEvent<SVGSVGElement>) => {
      if (event.pointerType !== 'touch') {
        return
      }

      touchPointsRef.current.set(event.pointerId, {
        x: event.clientX,
        y: event.clientY,
      })
      svgRef.current?.setPointerCapture(event.pointerId)

      const touchPair = getFirstTwoPoints(touchPointsRef.current)
      if (touchPair) {
        panRef.current = undefined
        pinchRef.current = getPinchGesture(touchPair, svgRef.current)
      }
    },
    [svgRef],
  )

  const beginPan = useCallback(
    (event: ReactPointerEvent<SVGElement>) => {
      if (event.button !== 0 || pinchRef.current) {
        return
      }

      const svg = svgRef.current
      const point = svg ? clientPointToSvg(svg, event.clientX, event.clientY) : undefined
      if (!svg || !point) {
        return
      }

      event.preventDefault()
      svg.focus({ preventScroll: true })
      svg.setPointerCapture(event.pointerId)
      panRef.current = { lastPoint: point, pointerId: event.pointerId }
    },
    [svgRef],
  )

  const handlePointerMove = useCallback(
    (event: ReactPointerEvent<SVGSVGElement>) => {
      if (event.pointerType === 'touch' && touchPointsRef.current.has(event.pointerId)) {
        touchPointsRef.current.set(event.pointerId, {
          x: event.clientX,
          y: event.clientY,
        })
      }

      const touchPair = getFirstTwoPoints(touchPointsRef.current)
      if (touchPair && pinchRef.current) {
        event.preventDefault()
        const svg = svgRef.current
        const nextPinch = getPinchGesture(touchPair, svg)
        if (!nextPinch) {
          return true
        }

        const previousPinch = pinchRef.current
        const currentState = stateRef.current
        const zoom = clamp(
          currentState.zoom * (nextPinch.distance / Math.max(previousPinch.distance, 1)),
          MIN_ZOOM,
          MAX_ZOOM,
        )
        const contentX = (previousPinch.midpoint.x - currentState.x) / currentState.zoom
        const contentY = (previousPinch.midpoint.y - currentState.y) / currentState.zoom
        updateState({
          x: nextPinch.midpoint.x - contentX * zoom,
          y: nextPinch.midpoint.y - contentY * zoom,
          zoom,
        })
        pinchRef.current = nextPinch
        return true
      }

      const pan = panRef.current
      const svg = svgRef.current
      if (!pan || !svg || pan.pointerId !== event.pointerId) {
        return false
      }

      event.preventDefault()
      const point = clientPointToSvg(svg, event.clientX, event.clientY)
      if (!point) {
        return true
      }

      const currentState = stateRef.current
      updateState({
        ...currentState,
        x: currentState.x + point.x - pan.lastPoint.x,
        y: currentState.y + point.y - pan.lastPoint.y,
      })
      pan.lastPoint = point
      return true
    },
    [svgRef, updateState],
  )

  const handlePointerEnd = useCallback(
    (event: ReactPointerEvent<SVGSVGElement>) => {
      const wasViewportGesture = Boolean(
        pinchRef.current || panRef.current?.pointerId === event.pointerId,
      )
      touchPointsRef.current.delete(event.pointerId)

      if (svgRef.current?.hasPointerCapture(event.pointerId)) {
        svgRef.current.releasePointerCapture(event.pointerId)
      }

      if (touchPointsRef.current.size < 2) {
        pinchRef.current = undefined
      }
      if (panRef.current?.pointerId === event.pointerId || wasViewportGesture) {
        panRef.current = undefined
      }

      return wasViewportGesture
    },
    [svgRef],
  )

  const isPinching = useCallback(() => pinchRef.current !== undefined, [])

  return {
    beginPan,
    handlePointerDownCapture,
    handlePointerEnd,
    handlePointerMove,
    handleWheel,
    isPinching,
    reset,
    state,
    transform: `translate(${state.x} ${state.y}) scale(${state.zoom})`,
    zoomIn,
    zoomOut,
  }
}

function clientPointToSvg(svg: SVGSVGElement, clientX: number, clientY: number) {
  const matrix = svg.getScreenCTM()?.inverse()
  if (!matrix) {
    return undefined
  }

  const point = new DOMPoint(clientX, clientY).matrixTransform(matrix)
  return { x: point.x, y: point.y }
}

function clientDeltaToSvg(svg: SVGSVGElement, deltaX: number, deltaY: number) {
  const matrix = svg.getScreenCTM()?.inverse()
  if (!matrix) {
    return undefined
  }

  const origin = new DOMPoint(0, 0).matrixTransform(matrix)
  const point = new DOMPoint(deltaX, deltaY).matrixTransform(matrix)
  return { x: point.x - origin.x, y: point.y - origin.y }
}

function getFirstTwoPoints(points: Map<number, Point>): [Point, Point] | undefined {
  const values = Array.from(points.values())
  return values.length >= 2 ? [values[0], values[1]] : undefined
}

function getPinchGesture(
  [first, second]: [Point, Point],
  svg?: SVGSVGElement | null,
): PinchGesture | undefined {
  if (!svg) {
    return undefined
  }

  const midpoint = clientPointToSvg(
    svg,
    (first.x + second.x) / 2,
    (first.y + second.y) / 2,
  )
  if (!midpoint) {
    return undefined
  }

  return {
    distance: Math.hypot(second.x - first.x, second.y - first.y),
    midpoint,
  }
}

function clampViewport(
  state: SvgViewportState,
  slideWidth: number,
  slideHeight: number,
) {
  const zoom = clamp(state.zoom, MIN_ZOOM, MAX_ZOOM)
  if (zoom <= 1) {
    return {
      x: roundViewportValue((slideWidth * (1 - zoom)) / 2),
      y: roundViewportValue((slideHeight * (1 - zoom)) / 2),
      zoom: roundViewportValue(zoom),
    }
  }

  return {
    x: roundViewportValue(clamp(state.x, slideWidth * (1 - zoom), 0)),
    y: roundViewportValue(clamp(state.y, slideHeight * (1 - zoom), 0)),
    zoom: roundViewportValue(zoom),
  }
}

function viewportStatesMatch(first: SvgViewportState, second: SvgViewportState) {
  return (
    Math.abs(first.x - second.x) < 0.01 &&
    Math.abs(first.y - second.y) < 0.01 &&
    Math.abs(first.zoom - second.zoom) < 0.001
  )
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max)
}

function roundViewportValue(value: number) {
  return Math.round(value * 1000) / 1000
}
