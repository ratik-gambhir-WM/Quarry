import { memo } from 'react'

import logoAsset from '../../assets/brand-logo.png'
import {
  getWestMonroeBrandFrameLayout,
  WEST_MONROE_BRAND_COLOR,
} from '../branding/BrandFrame'

export const SvgBrandFrame = memo(function SvgBrandFrame({
  height,
  width,
}: {
  height: number
  width: number
}) {
  const frame = getWestMonroeBrandFrameLayout(width, height)

  return (
    <g aria-label="West Monroe branded slide frame" pointerEvents="none">
      <rect
        data-brand-footer=""
        fill={`#${WEST_MONROE_BRAND_COLOR}`}
        height={frame.footer.h}
        width={frame.footer.w}
        x={frame.footer.x}
        y={frame.footer.y}
      />
      {frame.dots.map((dot, index) => (
        <rect
          data-brand-dot=""
          fill={`#${WEST_MONROE_BRAND_COLOR}`}
          height={dot.h}
          key={index}
          width={dot.w}
          x={dot.x}
          y={dot.y}
        />
      ))}
      <image
        data-brand-logo=""
        height={frame.logo.h}
        href={logoAsset}
        preserveAspectRatio="xMidYMid meet"
        width={frame.logo.w}
        x={frame.logo.x}
        y={frame.logo.y}
      />
    </g>
  )
})
