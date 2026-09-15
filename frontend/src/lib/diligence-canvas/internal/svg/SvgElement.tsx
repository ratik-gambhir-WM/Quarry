import { memo, type PointerEvent as ReactPointerEvent } from 'react'

import {
  getElementAccessibleLabel,
  type SlideElementRef,
} from '../canvas'
import { SvgImage } from './SvgImage'
import { getLinePath, getLineTransform, SvgLine } from './SvgLine'
import { SvgShape } from './SvgShape'
import { SvgTextBlock } from './SvgText'

export const SvgElement = memo(function SvgElement({
  definitionPrefix,
  elementRef,
  isEditing,
  isSelected,
  onDoubleClick,
  onFocus,
  onPointerDown,
  textFontScale,
}: {
  definitionPrefix: string
  elementRef: SlideElementRef
  isEditing: boolean
  isSelected: boolean
  onDoubleClick: (ref: SlideElementRef) => void
  onFocus: (ref: SlideElementRef) => void
  onPointerDown: (ref: SlideElementRef, event: ReactPointerEvent<SVGElement>) => void
  textFontScale?: number
}) {
  const element = elementRef.element
  const label = getElementAccessibleLabel(element)
  const supportsTextEditing = element.kind === 'shape' || element.kind === 'text'
  const title = supportsTextEditing
    ? `${label}. Click again, double-click, or press Enter to edit.`
    : label
  const clipId = `${definitionPrefix}-clip`
  const textClipId = `${definitionPrefix}-text-clip`

  if (element.kind === 'line') {
    return (
      <g
        aria-label={label}
        aria-pressed={isSelected}
        className="group cursor-grab outline-none active:cursor-grabbing"
        data-element-key={elementRef.key}
        onFocus={() => onFocus(elementRef)}
        onPointerDown={(event) => onPointerDown(elementRef, event)}
        opacity={element.opacity}
        role="button"
        tabIndex={0}
      >
        <SvgLine
          element={element}
          markerId={`${definitionPrefix}-marker`}
          maskId={`${definitionPrefix}-mask`}
        />
        <path
          className="cursor-[inherit] [pointer-events:all] group-focus:stroke-[rgba(20,81,225,0.72)] group-focus:stroke-2 group-focus:[vector-effect:non-scaling-stroke]"
          d={getLinePath(element)}
          fill="none"
          stroke="transparent"
          strokeWidth={Math.max(element.strokeWidth, 16)}
          transform={getLineTransform(element)}
        />
      </g>
    )
  }

  const transform = [
    `translate(${element.x} ${element.y})`,
    element.rotate ? `rotate(${element.rotate} ${element.w / 2} ${element.h / 2})` : '',
    element.flipH || element.flipV
      ? `translate(${element.flipH ? element.w : 0} ${element.flipV ? element.h : 0}) scale(${element.flipH ? -1 : 1} ${element.flipV ? -1 : 1})`
      : '',
  ].filter(Boolean).join(' ')
  return (
    <g
      aria-label={label}
      aria-keyshortcuts={supportsTextEditing ? 'Enter' : undefined}
      aria-pressed={isSelected}
      className="group cursor-grab outline-none active:cursor-grabbing"
      data-element-key={elementRef.key}
      onFocus={() => onFocus(elementRef)}
      onDoubleClick={
        supportsTextEditing
          ? (event) => {
              event.preventDefault()
              event.stopPropagation()
              onDoubleClick(elementRef)
            }
          : undefined
      }
      onPointerDown={(event) => onPointerDown(elementRef, event)}
      opacity={element.opacity}
      role="button"
      tabIndex={0}
      transform={transform}
    >
      <title>{title}</title>
      {element.kind === 'shape' ? (
        <SvgShape
          element={element}
          fontScale={textFontScale}
          hideText={isEditing}
          textClipId={textClipId}
        />
      ) : element.kind === 'text' ? (
        <SvgTextBlock
          clipId={textClipId}
          element={element}
          fontScale={textFontScale}
          hideText={isEditing}
          paintBox
        />
      ) : (
        <SvgImage clipId={clipId} element={element} />
      )}
      <rect
        className="cursor-[inherit] [pointer-events:all] group-focus:stroke-[rgba(20,81,225,0.72)] group-focus:stroke-2 group-focus:[vector-effect:non-scaling-stroke]"
        fill="transparent"
        height={element.h}
        width={element.w}
        x={0}
        y={0}
      />
    </g>
  )
})
