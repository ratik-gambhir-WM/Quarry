import type { ReactNode } from 'react'

import type { NormalizedShapeElement } from '../shared/PowerpointTypes'
import { toSvgColor } from './svgUtils'

export const SUPPORTED_SHAPE_NAMES = new Set([
  'can',
  'chevron',
  'decagon',
  'diamond',
  'dodecagon',
  'donut',
  'downArrow',
  'ellipse',
  'flowChartConnector',
  'flowChartDecision',
  'flowChartInputOutput',
  'flowChartMagneticDisk',
  'flowChartOffpageConnector',
  'flowChartProcess',
  'flowChartTerminator',
  'heart',
  'heptagon',
  'hexagon',
  'homePlate',
  'leftArrow',
  'leftRightArrow',
  'mathMinus',
  'mathPlus',
  'nonIsoscelesTrapezoid',
  'octagon',
  'parallelogram',
  'pentagon',
  'pie',
  'plus',
  'rect',
  'rightArrow',
  'roundRect',
  'rtTriangle',
  'star10',
  'star12',
  'star16',
  'star24',
  'star32',
  'star4',
  'star5',
  'star6',
  'star7',
  'star8',
  'trapezoid',
  'triangle',
  'upArrow',
  'upDownArrow',
])

export function renderShapePrimitive(element: NormalizedShapeElement): ReactNode {
  const common = {
    fill: toSvgColor(element.fill),
    fillOpacity: element.fillOpacity ?? 1,
    stroke: toSvgColor(element.stroke),
    strokeOpacity: element.strokeOpacity ?? 1,
    strokeWidth: element.strokeWidth,
    vectorEffect: 'non-scaling-stroke' as const,
  }
  const inset = element.strokeWidth / 2
  const width = Math.max(element.w - element.strokeWidth, 0)
  const height = Math.max(element.h - element.strokeWidth, 0)

  switch (element.shape) {
    case 'flowChartConnector':
    case 'ellipse':
      return (
        <ellipse
          {...common}
          cx={element.w / 2}
          cy={element.h / 2}
          rx={width / 2}
          ry={height / 2}
        />
      )
    case 'triangle':
      return (
        <polygon
          {...common}
          points={`${element.w / 2},${inset} ${element.w - inset},${element.h - inset} ${inset},${element.h - inset}`}
        />
      )
    case 'rtTriangle':
      return (
        <polygon
          {...common}
          points={`${inset},${inset} ${element.w - inset},${element.h - inset} ${inset},${element.h - inset}`}
        />
      )
    case 'pie':
      return (
        <path
          {...common}
          d={`M ${element.w / 2} ${element.h / 2} L ${element.w / 2} ${inset} A ${width / 2} ${height / 2} 0 0 1 ${element.w - inset} ${element.h / 2} Z`}
        />
      )
    case 'flowChartDecision':
    case 'diamond':
      return (
        <polygon
          {...common}
          points={`${element.w / 2},${inset} ${element.w - inset},${element.h / 2} ${element.w / 2},${element.h - inset} ${inset},${element.h / 2}`}
        />
      )
    case 'parallelogram':
    case 'flowChartInputOutput':
      return <polygon {...common} points={points([
        [0.2, 0], [1, 0], [0.8, 1], [0, 1],
      ], element, inset)} />
    case 'trapezoid':
    case 'nonIsoscelesTrapezoid':
      return <polygon {...common} points={points([
        [0.2, 0], [0.8, 0], [1, 1], [0, 1],
      ], element, inset)} />
    case 'pentagon':
    case 'flowChartOffpageConnector':
      return <polygon {...common} points={regularPolygonPoints(5, element, inset)} />
    case 'hexagon':
      return <polygon {...common} points={regularPolygonPoints(6, element, inset)} />
    case 'heptagon':
      return <polygon {...common} points={regularPolygonPoints(7, element, inset)} />
    case 'octagon':
      return <polygon {...common} points={regularPolygonPoints(8, element, inset)} />
    case 'decagon':
      return <polygon {...common} points={regularPolygonPoints(10, element, inset)} />
    case 'dodecagon':
      return <polygon {...common} points={regularPolygonPoints(12, element, inset)} />
    case 'chevron':
      return (
        <polygon
          {...common}
          points={`${inset},${inset} ${element.w * 0.78},${inset} ${element.w - inset},${element.h / 2} ${element.w * 0.78},${element.h - inset} ${inset},${element.h - inset} ${element.w * 0.22},${element.h / 2}`}
        />
      )
    case 'homePlate':
      return <polygon {...common} points={points([
        [0, 0], [0.72, 0], [1, 0.5], [0.72, 1], [0, 1],
      ], element, inset)} />
    case 'rightArrow':
      return <polygon {...common} points={arrowPoints('right', element, inset)} />
    case 'leftArrow':
      return <polygon {...common} points={arrowPoints('left', element, inset)} />
    case 'upArrow':
      return <polygon {...common} points={arrowPoints('up', element, inset)} />
    case 'downArrow':
      return <polygon {...common} points={arrowPoints('down', element, inset)} />
    case 'leftRightArrow':
      return <polygon {...common} points={points([
        [0, 0.5], [0.2, 0.18], [0.2, 0.36], [0.8, 0.36], [0.8, 0.18],
        [1, 0.5], [0.8, 0.82], [0.8, 0.64], [0.2, 0.64], [0.2, 0.82],
      ], element, inset)} />
    case 'upDownArrow':
      return <polygon {...common} points={points([
        [0.5, 0], [0.82, 0.2], [0.64, 0.2], [0.64, 0.8], [0.82, 0.8],
        [0.5, 1], [0.18, 0.8], [0.36, 0.8], [0.36, 0.2], [0.18, 0.2],
      ], element, inset)} />
    case 'plus':
    case 'mathPlus':
      return <polygon {...common} points={points([
        [0.36, 0], [0.64, 0], [0.64, 0.36], [1, 0.36], [1, 0.64], [0.64, 0.64],
        [0.64, 1], [0.36, 1], [0.36, 0.64], [0, 0.64], [0, 0.36], [0.36, 0.36],
      ], element, inset)} />
    case 'mathMinus':
      return <rect {...common} height={height * 0.24} width={width} x={inset} y={element.h * 0.38} />
    case 'star4':
    case 'star5':
    case 'star6':
    case 'star7':
    case 'star8':
    case 'star10':
    case 'star12':
    case 'star16':
    case 'star24':
    case 'star32':
      return (
        <polygon
          {...common}
          points={starPoints(Number(element.shape.slice(4)), element, inset)}
        />
      )
    case 'heart':
      return (
        <path
          {...common}
          d={`M ${element.w / 2} ${element.h - inset} C ${element.w * 0.42} ${element.h * 0.78}, ${inset} ${element.h * 0.58}, ${inset} ${element.h * 0.3} C ${inset} ${element.h * 0.02}, ${element.w * 0.34} ${-element.h * 0.02}, ${element.w / 2} ${element.h * 0.22} C ${element.w * 0.66} ${-element.h * 0.02}, ${element.w - inset} ${element.h * 0.02}, ${element.w - inset} ${element.h * 0.3} C ${element.w - inset} ${element.h * 0.58}, ${element.w * 0.58} ${element.h * 0.78}, ${element.w / 2} ${element.h - inset} Z`}
        />
      )
    case 'donut':
      return (
        <path
          {...common}
          d={`M ${element.w / 2} ${inset} A ${width / 2} ${height / 2} 0 1 1 ${element.w / 2 - 0.01} ${inset} Z M ${element.w / 2} ${element.h * 0.27} A ${element.w * 0.23} ${element.h * 0.23} 0 1 0 ${element.w / 2 - 0.01} ${element.h * 0.27} Z`}
          fillRule="evenodd"
        />
      )
    case 'can':
    case 'flowChartMagneticDisk': {
      const capHeight = Math.min(element.h * 0.22, 18)
      return (
        <>
          <path
            {...common}
            d={`M ${inset} ${capHeight / 2} C ${inset} ${-capHeight / 6}, ${element.w - inset} ${-capHeight / 6}, ${element.w - inset} ${capHeight / 2} L ${element.w - inset} ${element.h - capHeight / 2} C ${element.w - inset} ${element.h + capHeight / 6}, ${inset} ${element.h + capHeight / 6}, ${inset} ${element.h - capHeight / 2} Z`}
          />
          <ellipse
            cx={element.w / 2}
            cy={capHeight / 2}
            fill="none"
            rx={width / 2}
            ry={capHeight / 2}
            stroke={common.stroke}
            strokeWidth={common.strokeWidth}
            vectorEffect="non-scaling-stroke"
          />
        </>
      )
    }
    case 'roundRect':
    case 'flowChartTerminator':
    case 'rect':
    case 'flowChartProcess':
      return (
        <rect
          {...common}
          height={height}
          rx={Math.max(
            element.borderRadius ||
              (element.shape === 'roundRect' || element.shape === 'flowChartTerminator' ? 18 : 0),
            0,
          )}
          width={width}
          x={inset}
          y={inset}
        />
      )
    default:
      return (
        <rect
          fill="rgba(255, 0, 80, 0.08)"
          height={height}
          stroke="#ff0050"
          strokeDasharray="6 4"
          strokeWidth={Math.max(element.strokeWidth, 1)}
          width={width}
          x={inset}
          y={inset}
        />
      )
  }
}

function points(
  normalizedPoints: Array<[number, number]>,
  element: NormalizedShapeElement,
  inset: number,
) {
  const width = Math.max(element.w - inset * 2, 0)
  const height = Math.max(element.h - inset * 2, 0)
  return normalizedPoints
    .map(([x, y]) => `${inset + x * width},${inset + y * height}`)
    .join(' ')
}

function regularPolygonPoints(sides: number, element: NormalizedShapeElement, inset: number) {
  return Array.from({ length: sides }, (_, index) => {
    const angle = -Math.PI / 2 + (Math.PI * 2 * index) / sides
    return [0.5 + Math.cos(angle) * 0.5, 0.5 + Math.sin(angle) * 0.5] as [number, number]
  }).map(([x, y]) => {
    const width = Math.max(element.w - inset * 2, 0)
    const height = Math.max(element.h - inset * 2, 0)
    return `${inset + x * width},${inset + y * height}`
  }).join(' ')
}

function starPoints(pointsCount: number, element: NormalizedShapeElement, inset: number) {
  return Array.from({ length: pointsCount * 2 }, (_, index) => {
    const angle = -Math.PI / 2 + (Math.PI * index) / pointsCount
    const radius = index % 2 === 0 ? 0.5 : 0.22
    return [0.5 + Math.cos(angle) * radius, 0.5 + Math.sin(angle) * radius] as [number, number]
  }).map(([x, y]) => {
    const width = Math.max(element.w - inset * 2, 0)
    const height = Math.max(element.h - inset * 2, 0)
    return `${inset + x * width},${inset + y * height}`
  }).join(' ')
}

function arrowPoints(
  direction: 'down' | 'left' | 'right' | 'up',
  element: NormalizedShapeElement,
  inset: number,
) {
  const right: Array<[number, number]> = [
    [0, 0.32], [0.62, 0.32], [0.62, 0], [1, 0.5], [0.62, 1], [0.62, 0.68], [0, 0.68],
  ]
  const transformed = right.map(([x, y]) => {
    if (direction === 'left') return [1 - x, y] as [number, number]
    if (direction === 'up') return [y, 1 - x] as [number, number]
    if (direction === 'down') return [y, x] as [number, number]
    return [x, y] as [number, number]
  })
  return points(transformed, element, inset)
}
