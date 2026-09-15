import { memo } from 'react'

import type { NormalizedImageElement } from '../shared/PowerpointTypes'

export const SvgImage = memo(function SvgImage({
  clipId,
  element,
}: {
  clipId: string
  element: NormalizedImageElement
}) {
  const placement = getImagePlacement(element)

  return (
    <>
      <title>{element.altText || 'Slide image'}</title>
      <defs>
        <clipPath id={clipId}>
          <rect
            height={element.h}
            rx={element.borderRadius}
            width={element.w}
            x={0}
            y={0}
          />
        </clipPath>
      </defs>
      <image
        clipPath={`url(#${clipId})`}
        height={placement.height}
        href={element.src}
        preserveAspectRatio={placement.preserveAspectRatio}
        width={placement.width}
        x={placement.x}
        y={placement.y}
      />
    </>
  )
})

function getPreserveAspectRatio(fit: NormalizedImageElement['fit']) {
  if (fit === 'stretch') {
    return 'none'
  }
  if (fit === 'cover') {
    return 'xMidYMid slice'
  }
  return 'xMidYMid meet'
}

function getImagePlacement(element: NormalizedImageElement) {
  if (!element.crop) {
    return {
      height: element.h,
      preserveAspectRatio: getPreserveAspectRatio(element.fit),
      width: element.w,
      x: 0,
      y: 0,
    }
  }

  const visibleWidth = Math.max(1 - element.crop.left - element.crop.right, 0.001)
  const visibleHeight = Math.max(1 - element.crop.top - element.crop.bottom, 0.001)
  const width = element.w / visibleWidth
  const height = element.h / visibleHeight

  return {
    height,
    preserveAspectRatio: 'none',
    width,
    x: -width * element.crop.left,
    y: -height * element.crop.top,
  }
}
