import { useId, useMemo } from 'react'

import type { JsonValue } from '../shared/PowerpointTypes'
import { buildSlideCanvasModel } from '../canvas'
import { SvgSlide } from './SvgSlide'
import { getConsistentSvgTextFontScales } from './svgTextLayout'
import { sanitizeSvgId } from './svgUtils'

const EMPTY_SELECTED_KEYS = new Set<string>()

export function SvgSlidePreview({
  className,
  input,
  showBranding,
}: {
  className?: string
  input: JsonValue
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
    return (
      <span className={['font-sans text-sm text-slate-500', className].filter(Boolean).join(' ')}>
        Preview unavailable
      </span>
    )
  }

  const shouldShowBranding = showBranding ?? model.presentation?.meta.showBranding ?? true
  const clipId = `${canvasId}-root-clip`

  return (
    <svg
      aria-hidden="true"
      className={['pointer-events-none block', className].filter(Boolean).join(' ')}
      preserveAspectRatio="xMidYMid meet"
      viewBox={`0 0 ${slide.width} ${slide.height}`}
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
  )
}
