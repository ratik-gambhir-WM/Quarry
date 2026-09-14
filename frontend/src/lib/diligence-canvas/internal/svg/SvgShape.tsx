import { memo } from 'react'

import type { NormalizedShapeElement } from '../shared/PowerpointTypes'
import { renderShapePrimitive } from './shapes'
import { SvgTextBlock } from './SvgText'

export const SvgShape = memo(function SvgShape({
  element,
  fontScale,
  hideText = false,
  textClipId,
}: {
  element: NormalizedShapeElement
  fontScale?: number
  hideText?: boolean
  textClipId: string
}) {
  return (
    <>
      {renderShapePrimitive(element)}
      {!hideText && (element.label || element.textRuns.length) ? (
        <SvgTextBlock clipId={textClipId} element={element} fontScale={fontScale} />
      ) : null}
    </>
  )
})
