import { memo, type PointerEvent as ReactPointerEvent } from 'react'

import type { SlideElementRef } from '../canvas'
import { getLinePath, getLineTransform } from './SvgLine'
import type { ResizeHandle } from './useSvgInteraction'

const HANDLE_SPECS: Array<{
  handle: ResizeHandle
  x: number
  y: number
}> = [
  { handle: 'nw', x: 0, y: 0 },
  { handle: 'n', x: 0.5, y: 0 },
  { handle: 'ne', x: 1, y: 0 },
  { handle: 'e', x: 1, y: 0.5 },
  { handle: 'se', x: 1, y: 1 },
  { handle: 's', x: 0.5, y: 1 },
  { handle: 'sw', x: 0, y: 1 },
  { handle: 'w', x: 0, y: 0.5 },
]

export const SvgSelection = memo(function SvgSelection({
  elementRef,
  onLinePointPointerDown,
  onResizePointerDown,
  showHandles,
  zoom,
}: {
  elementRef: SlideElementRef
  onLinePointPointerDown: (
    ref: SlideElementRef,
    point: 'end' | 'start',
    event: ReactPointerEvent<SVGElement>,
  ) => void
  onResizePointerDown: (
    ref: SlideElementRef,
    handle: ResizeHandle,
    event: ReactPointerEvent<SVGElement>,
  ) => void
  showHandles: boolean
  zoom: number
}) {
  const element = elementRef.element
  if (element.kind === 'line') {
    return (
      <g className="pointer-events-none" transform={getLineTransform(element)}>
        <path
          className="fill-none stroke-[rgba(20,81,225,0.72)] stroke-[1.5] [stroke-dasharray:5_4] [vector-effect:non-scaling-stroke]"
          d={getLinePath(element)}
        />
        {showHandles ? (
          <>
            <LineHandle
              label="Move line start"
              onPointerDown={(event) => onLinePointPointerDown(elementRef, 'start', event)}
              x={element.x1}
              y={element.y1}
              zoom={zoom}
            />
            <LineHandle
              label="Move line end"
              onPointerDown={(event) => onLinePointPointerDown(elementRef, 'end', event)}
              x={element.x2}
              y={element.y2}
              zoom={zoom}
            />
          </>
        ) : null}
      </g>
    )
  }

  const transform = `translate(${element.x} ${element.y})${element.rotate ? ` rotate(${element.rotate} ${element.w / 2} ${element.h / 2})` : ''}`
  return (
    <g className="pointer-events-none" transform={transform}>
      <rect
        className="fill-none stroke-[rgba(20,81,225,0.72)] stroke-[1.5] [vector-effect:non-scaling-stroke]"
        height={element.h}
        width={element.w}
        x={0}
        y={0}
      />
      {showHandles ? HANDLE_SPECS.map((handle) => (
        <rect
          aria-label={`Resize ${handle.handle}`}
          className={resizeHandleClasses(handle.handle)}
          height={10 / zoom}
          key={handle.handle}
          onPointerDown={(event) => onResizePointerDown(elementRef, handle.handle, event)}
          role="button"
          width={10 / zoom}
          x={element.w * handle.x - 5 / zoom}
          y={element.h * handle.y - 5 / zoom}
        />
      )) : null}
    </g>
  )
})

function LineHandle({
  label,
  onPointerDown,
  x,
  y,
  zoom,
}: {
  label: string
  onPointerDown: (event: ReactPointerEvent<SVGElement>) => void
  x: number
  y: number
  zoom: number
}) {
  return (
    <circle
      aria-label={label}
      className="cursor-grab fill-[#1451e1] stroke-white stroke-[1.5] [pointer-events:all] [vector-effect:non-scaling-stroke] active:cursor-grabbing"
      cx={x}
      cy={y}
      onPointerDown={onPointerDown}
      r={7 / zoom}
      role="button"
    />
  )
}

function resizeHandleClasses(handle: ResizeHandle) {
  const cursor = {
    e: 'cursor-ew-resize',
    n: 'cursor-ns-resize',
    ne: 'cursor-nesw-resize',
    nw: 'cursor-nwse-resize',
    s: 'cursor-ns-resize',
    se: 'cursor-nwse-resize',
    sw: 'cursor-nesw-resize',
    w: 'cursor-ew-resize',
  }[handle]

  return `${cursor} fill-[#1451e1] stroke-white stroke-[1.5] [pointer-events:all] [vector-effect:non-scaling-stroke]`
}
