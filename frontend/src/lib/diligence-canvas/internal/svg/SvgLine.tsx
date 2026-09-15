import { memo } from 'react'

import type { NormalizedLineElement } from '../shared/PowerpointTypes'
import { toSvgColor } from './svgUtils'

export const SvgLine = memo(function SvgLine({
  element,
  markerId,
  maskId,
}: {
  element: NormalizedLineElement
  markerId: string
  maskId: string
}) {
  const hasStartArrow = !!element.beginArrow && element.beginArrow !== 'none'
  const hasEndArrow = element.endArrow !== 'none'
  const hasMask = element.occlusionRects.length > 0
  const path = getLinePath(element)

  return (
    <g transform={getLineTransform(element)}>
      <defs>
        {hasEndArrow ? (
          <marker
            id={markerId}
            markerHeight="10"
            markerUnits="userSpaceOnUse"
            markerWidth="10"
            orient="auto"
            refX="9"
            refY="5"
            viewBox="0 0 10 10"
          >
            {renderArrowMarker(element.endArrow, element.stroke)}
          </marker>
        ) : null}
        {hasStartArrow ? (
          <marker
            id={`${markerId}-start`}
            markerHeight="10"
            markerUnits="userSpaceOnUse"
            markerWidth="10"
            orient="auto-start-reverse"
            refX="9"
            refY="5"
            viewBox="0 0 10 10"
          >
            {renderArrowMarker(element.beginArrow ?? 'none', element.stroke)}
          </marker>
        ) : null}
        {hasMask ? (
          <mask id={maskId} maskUnits="userSpaceOnUse" x="-4096" y="-4096" width="8192" height="8192">
            <rect fill="white" height="8192" width="8192" x="-4096" y="-4096" />
            {element.occlusionRects.map((rect, index) => (
              <rect
                fill="black"
                height={rect.h + 3}
                key={`${index}-${rect.x}-${rect.y}`}
                width={rect.w + 3}
                x={rect.x - 1.5}
                y={rect.y - 1.5}
              />
            ))}
          </mask>
        ) : null}
      </defs>
      <path
        d={path}
        fill="none"
        markerEnd={hasEndArrow ? `url(#${markerId})` : undefined}
        markerStart={hasStartArrow ? `url(#${markerId}-start)` : undefined}
        mask={hasMask ? `url(#${maskId})` : undefined}
        stroke={toSvgColor(element.stroke)}
        strokeOpacity={element.strokeOpacity ?? 1}
        strokeDasharray={getStrokeDasharray(element)}
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth={element.strokeWidth}
        vectorEffect="non-scaling-stroke"
      />
    </g>
  )
})

export function getLinePath(element: NormalizedLineElement) {
  if (element.lineType === 'elbow') {
    const bendX = element.x1 + (element.x2 - element.x1) * element.bendPosition
    return `M ${element.x1} ${element.y1} L ${bendX} ${element.y1} L ${bendX} ${element.y2} L ${element.x2} ${element.y2}`
  }
  return `M ${element.x1} ${element.y1} L ${element.x2} ${element.y2}`
}

export function getLineTransform(element: NormalizedLineElement) {
  if (!element.rotate) {
    return undefined
  }

  const midX = (element.x1 + element.x2) / 2
  const midY = (element.y1 + element.y2) / 2
  return `rotate(${element.rotate} ${midX} ${midY})`
}

function renderArrowMarker(arrow: NonNullable<NormalizedLineElement['beginArrow']>, stroke: string) {
  const color = toSvgColor(stroke)
  if (arrow === 'diamond') {
    return <path d="M 0 5 L 5 0 L 10 5 L 5 10 Z" fill={color} />
  }
  if (arrow === 'oval') {
    return <circle cx="5" cy="5" fill={color} r="4" />
  }
  if (arrow === 'arrow') {
    return <path d="M 1 1 L 9 5 L 1 9 L 3.5 5 Z" fill={color} />
  }
  if (arrow === 'stealth') {
    return <path d="M 0 1 L 10 5 L 0 9 L 3 5 Z" fill={color} />
  }
  return <path d="M 0 0 L 10 5 L 0 10 Z" fill={color} />
}

function getStrokeDasharray(element: NormalizedLineElement) {
  if (element.dash === 'dash') {
    return `${element.strokeWidth * 5} ${element.strokeWidth * 3}`
  }
  if (element.dash === 'dot') {
    return `${element.strokeWidth} ${element.strokeWidth * 3}`
  }
  return undefined
}
