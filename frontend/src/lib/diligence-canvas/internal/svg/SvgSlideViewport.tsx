import type {
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
  ReactNode,
  RefObject,
  CSSProperties,
  WheelEvent as ReactWheelEvent,
} from 'react'

import type { NormalizedSlide } from '../shared/PowerpointTypes'

export function SvgSlideViewport({
  children,
  clipId,
  contentRef,
  contentTransform,
  descriptionId,
  id,
  onKeyDown,
  onPointerCancel,
  onPointerDownCapture,
  onPointerMove,
  onPointerUp,
  onWheel,
  slide,
  svgRef,
  viewportSize,
}: {
  children: ReactNode
  clipId: string
  contentRef: RefObject<SVGGElement | null>
  contentTransform: string
  descriptionId: string
  id: string
  onKeyDown: (event: ReactKeyboardEvent<SVGSVGElement>) => void
  onPointerCancel: (event: ReactPointerEvent<SVGSVGElement>) => void
  onPointerDownCapture: (event: ReactPointerEvent<SVGSVGElement>) => void
  onPointerMove: (event: ReactPointerEvent<SVGSVGElement>) => void
  onPointerUp: (event: ReactPointerEvent<SVGSVGElement>) => void
  onWheel: (event: ReactWheelEvent<SVGSVGElement>) => void
  slide: NormalizedSlide
  svgRef: RefObject<SVGSVGElement | null>
  viewportSize?: { height: number; width: number }
}) {
  return (
    <svg
      aria-label={`${slide.name} editable slide canvas`}
      aria-describedby={descriptionId}
      className="block h-auto max-h-full w-[calc(100%-32px)] max-w-full touch-none bg-[var(--slide-canvas-bg)] shadow-[0_18px_45px_rgba(26,32,56,0.18),0_0_0_1px_rgba(27,39,67,0.12)] outline-none"
      id={id}
      onKeyDown={onKeyDown}
      onPointerCancel={onPointerCancel}
      onPointerDownCapture={onPointerDownCapture}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onWheel={onWheel}
      preserveAspectRatio="xMidYMid meet"
      ref={svgRef}
      role="application"
      style={
        viewportSize
          ? ({ height: viewportSize.height, width: viewportSize.width } as CSSProperties)
          : undefined
      }
      tabIndex={0}
      viewBox={`0 0 ${slide.width} ${slide.height}`}
    >
      <defs>
        <clipPath id={clipId}>
          <rect height={slide.height} width={slide.width} x={0} y={0} />
        </clipPath>
      </defs>
      <g clipPath={`url(#${clipId})`}>
        <g data-slide-viewport-content="" ref={contentRef} transform={contentTransform}>
          {children}
        </g>
      </g>
    </svg>
  )
}
