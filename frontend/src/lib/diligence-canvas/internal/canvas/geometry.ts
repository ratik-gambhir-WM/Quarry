import type { NormalizedElement, NormalizedLineElement } from '../shared/PowerpointTypes'

export type BoxGeometry = {
  h: number
  w: number
  x: number
  y: number
}

export type LineGeometry = {
  x1: number
  x2: number
  y1: number
  y2: number
}

export type ElementGeometry = BoxGeometry | LineGeometry

type Point = {
  x: number
  y: number
}

export type RenderedLinePoints = {
  x1: number
  x2: number
  y1: number
  y2: number
}

export function getRenderedLinePoints(element: NormalizedLineElement): RenderedLinePoints {
  const points = getRenderedLinePathPoints(element)
  const start = points[0]
  const end = points[points.length - 1]

  return {
    x1: start.x,
    x2: end.x,
    y1: start.y,
    y2: end.y,
  }
}

function getRenderedLinePathPoints(element: NormalizedLineElement): Point[] {
  const bendX = element.x1 + (element.x2 - element.x1) * element.bendPosition
  const points = element.lineType === 'elbow'
    ? [
        { x: element.x1, y: element.y1 },
        { x: bendX, y: element.y1 },
        { x: bendX, y: element.y2 },
        { x: element.x2, y: element.y2 },
      ]
    : [
        { x: element.x1, y: element.y1 },
        { x: element.x2, y: element.y2 },
      ]

  if (!element.rotate) {
    return points
  }

  const centerX = (element.x1 + element.x2) / 2
  const centerY = (element.y1 + element.y2) / 2
  return points.map((point) => rotatePoint(point.x, point.y, centerX, centerY, element.rotate))
}

export function getElementGeometry(element: NormalizedElement): BoxGeometry {
  if (element.kind !== 'line') {
    return {
      h: Math.max(element.h, 1),
      w: Math.max(element.w, 1),
      x: element.x,
      y: element.y,
    }
  }

  const points = getRenderedLinePathPoints(element)
  const xCoordinates = points.map((point) => point.x)
  const yCoordinates = points.map((point) => point.y)
  const minX = Math.min(...xCoordinates)
  const maxX = Math.max(...xCoordinates)
  const minY = Math.min(...yCoordinates)
  const maxY = Math.max(...yCoordinates)
  return {
    h: Math.max(maxY - minY, Math.max(element.strokeWidth, 1)),
    w: Math.max(maxX - minX, Math.max(element.strokeWidth, 1)),
    x: minX,
    y: minY,
  }
}

function rotatePoint(
  x: number,
  y: number,
  centerX: number,
  centerY: number,
  degrees: number,
): Point {
  const radians = (degrees * Math.PI) / 180
  const cosine = Math.cos(radians)
  const sine = Math.sin(radians)
  const offsetX = x - centerX
  const offsetY = y - centerY

  return {
    x: centerX + offsetX * cosine - offsetY * sine,
    y: centerY + offsetX * sine + offsetY * cosine,
  }
}

export function roundCoordinate(value: number) {
  return Math.round(value * 100) / 100
}

export function getElementAccessibleLabel(element: NormalizedElement) {
  if (element.kind === 'text') {
    return element.text.trim() || 'Text element'
  }

  if (element.kind === 'shape') {
    return element.label.trim() || `${element.shape} shape`
  }

  if (element.kind === 'image') {
    return element.altText.trim() || 'Image element'
  }

  return `${element.lineType} line`
}
