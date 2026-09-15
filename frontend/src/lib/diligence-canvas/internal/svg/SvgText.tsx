import { memo, useMemo } from 'react'

import { DEFAULT_FONT_FACE } from '../shared/PowerpointConstants'
import type {
  NormalizedShapeElement,
  NormalizedTextElement,
} from '../shared/PowerpointTypes'
import { getSvgTextContent } from './svgTextLayout'
import { toSvgColor } from './svgUtils'

type SvgTextBlockProps = {
  clipId: string
  element: NormalizedShapeElement | NormalizedTextElement
  fontScale?: number
  hideText?: boolean
  paintBox?: boolean
}

export const SvgTextBlock = memo(function SvgTextBlock({
  clipId,
  element,
  fontScale,
  hideText = false,
  paintBox = false,
}: SvgTextBlockProps) {
  const { layout, runs: displayRuns, text } = useMemo(
    () => getSvgTextContent(element, fontScale),
    [element, fontScale],
  )

  return (
    <>
      {paintBox ? (
        <rect
          fill={toSvgColor(element.fill)}
          fillOpacity={element.fillOpacity ?? 1}
          height={Math.max(element.h - element.strokeWidth, 0)}
          rx={element.borderRadius}
          stroke={toSvgColor(element.stroke)}
          strokeOpacity={element.strokeOpacity ?? 1}
          strokeWidth={element.strokeWidth}
          vectorEffect="non-scaling-stroke"
          width={Math.max(element.w - element.strokeWidth, 0)}
          x={element.strokeWidth / 2}
          y={element.strokeWidth / 2}
        />
      ) : null}
      <defs>
        <clipPath id={clipId}>
          <rect height={element.h} width={element.w} x={0} y={0} />
        </clipPath>
      </defs>
      {!hideText && (text || displayRuns.length) ? (
        <text
          clipPath={`url(#${clipId})`}
          dominantBaseline="alphabetic"
          pointerEvents="none"
          textRendering="geometricPrecision"
        >
          {layout.lines.flatMap((line, lineIndex) =>
            line.segments.map((segment, segmentIndex) => (
              <tspan
                fill={toSvgColor(segment.run.color)}
                fontFamily={segment.run.fontFace || DEFAULT_FONT_FACE}
                fontSize={Math.max(segment.run.fontSize * layout.fontScale, 5)}
                fontStyle={segment.run.italic ? 'italic' : 'normal'}
                fontWeight={segment.run.bold ? 700 : 400}
                key={`${lineIndex}-${segmentIndex}`}
                textDecoration={segment.run.underline ? 'underline' : undefined}
                x={segment.x}
                xmlSpace="preserve"
                y={line.baseline}
              >
                {segment.text}
              </tspan>
            )),
          )}
        </text>
      ) : null}
    </>
  )
})
