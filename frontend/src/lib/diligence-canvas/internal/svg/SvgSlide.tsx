import { memo, type PointerEvent as ReactPointerEvent } from 'react'

import type { NormalizedSlide } from '../shared/PowerpointTypes'
import type { SlideElementRef } from '../canvas'
import { SvgBrandFrame } from './SvgBrandFrame'
import { SvgElement } from './SvgElement'
import { sanitizeSvgId, toSvgColor } from './svgUtils'

const EMPTY_TEXT_FONT_SCALES = new Map<string, number>()

export const SvgSlide = memo(function SvgSlide({
  canvasId,
  editingKey,
  elementRefs,
  onElementFocus,
  onElementDoubleClick,
  onElementPointerDown,
  onSlidePointerDown,
  selectedKeys,
  showBranding,
  slide,
  textFontScales = EMPTY_TEXT_FONT_SCALES,
}: {
  canvasId: string
  editingKey?: string
  elementRefs: SlideElementRef[]
  onElementFocus: (ref: SlideElementRef) => void
  onElementDoubleClick: (ref: SlideElementRef) => void
  onElementPointerDown: (ref: SlideElementRef, event: ReactPointerEvent<SVGElement>) => void
  onSlidePointerDown: (event: ReactPointerEvent<SVGRectElement>) => void
  selectedKeys: ReadonlySet<string>
  showBranding: boolean
  slide: NormalizedSlide
  textFontScales?: ReadonlyMap<string, number>
}) {
  return (
    <>
      <rect
        aria-label={slide.name}
        fill={toSvgColor(slide.backgroundColor)}
        height={slide.height}
        onPointerDown={onSlidePointerDown}
        width={slide.width}
        x={0}
        y={0}
      />
      {showBranding ? (
        <SvgBrandFrame height={slide.height} width={slide.width} />
      ) : null}
      {elementRefs.map((elementRef) => (
        <SvgElement
          definitionPrefix={`${canvasId}-${sanitizeSvgId(elementRef.key)}`}
          elementRef={elementRef}
          isEditing={editingKey === elementRef.key}
          isSelected={selectedKeys.has(elementRef.key)}
          key={elementRef.key}
          onDoubleClick={onElementDoubleClick}
          onFocus={onElementFocus}
          onPointerDown={onElementPointerDown}
          textFontScale={textFontScales.get(elementRef.key)}
        />
      ))}
    </>
  )
})
