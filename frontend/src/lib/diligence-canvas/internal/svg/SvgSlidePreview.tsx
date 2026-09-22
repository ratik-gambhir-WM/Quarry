import { useId, useMemo } from 'react'

import type { JsonValue } from '../shared/PowerpointTypes'
import { buildSlideCanvasModel } from '../canvas'
import { SvgSlide } from './SvgSlide'
import { getConsistentSvgTextFontScales } from './svgTextLayout'
import { sanitizeSvgId } from './svgUtils'

const EMPTY_SELECTED_KEYS = new Set<string>()

export function SvgSlidePreview({
  input,
  maxDimension,
  showBranding,
}: {
  input: JsonValue
  maxDimension: number
  showBranding?: boolean
}) {
  const model = useMemo(() => buildSlideCanvasModel(input), [input])
  const reactId = useId()
  const canvasId = useMemo(() => `svg-preview-${sanitizeSvgId(reactId)}`, [reactId])
  const textFontScales = useMemo(
    () => getConsistentSvgTextFontScales(model.elementRefs),
    [model.elementRefs],
  )
  const slide = model.slide

  if (!slide) {
    return <main data-preview-state="error">Unable to render this slide template.</main>
  }

  const scale = maxDimension / Math.max(slide.width, slide.height)
  const renderedWidth = Math.max(1, Math.round(slide.width * scale))
  const renderedHeight = Math.max(1, Math.round(slide.height * scale))
  const shouldShowBranding = showBranding ?? model.presentation?.meta.showBranding ?? true
  const clipId = `${canvasId}-root-clip`

  return (
    <main data-preview-state="ready">
      <svg
        aria-hidden="true"
        data-template-preview-surface=""
        height={renderedHeight}
        preserveAspectRatio="xMidYMid meet"
        style={{ display: 'block' }}
        viewBox={`0 0 ${slide.width} ${slide.height}`}
        width={renderedWidth}
      >
        <defs>
          <clipPath id={clipId}>
            <rect height={slide.height} width={slide.width} x={0} y={0} />
          </clipPath>
        </defs>
        <g clipPath={`url(#${clipId})`}>
          <SvgSlide
            canvasId={canvasId}
            elementRefs={model.elementRefs}
            onElementDoubleClick={() => {}}
            onElementFocus={() => {}}
            onElementPointerDown={() => {}}
            onSlidePointerDown={() => {}}
            selectedKeys={EMPTY_SELECTED_KEYS}
            showBranding={shouldShowBranding}
            slide={slide}
            textFontScales={textFontScales}
          />
        </g>
      </svg>
    </main>
  )
}
