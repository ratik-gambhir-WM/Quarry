export const WEST_MONROE_BRAND_COLOR = 'E8EEF8'

const REFERENCE_WIDTH = 1280
const REFERENCE_HEIGHT = 720
const FOOTER_HEIGHT = 75

const REFERENCE_LOGO = {
  x: 48.33,
  y: 664.08,
  w: 153,
  h: 32.02,
} as const

const REFERENCE_DOTS = Array.from({ length: 15 * 17 }, (_, index) => ({
  x: 15 + (index % 15) * 19,
  y: 6 + Math.floor(index / 15) * 19,
  w: 2.5,
  h: 2.5,
}))

export type BrandFrameRect = Readonly<{
  x: number
  y: number
  w: number
  h: number
}>

export type WestMonroeBrandFrameLayout = Readonly<{
  dots: readonly BrandFrameRect[]
  footer: BrandFrameRect
  logo: BrandFrameRect
}>

export function getWestMonroeBrandFrameLayout(
  width: number,
  height: number,
): WestMonroeBrandFrameLayout {
  const scaleX = width / REFERENCE_WIDTH
  const scaleY = height / REFERENCE_HEIGHT
  const scaleRect = (rect: BrandFrameRect): BrandFrameRect => ({
    x: rect.x * scaleX,
    y: rect.y * scaleY,
    w: rect.w * scaleX,
    h: rect.h * scaleY,
  })

  return {
    dots: REFERENCE_DOTS.map(scaleRect),
    footer: {
      x: 0,
      y: height - FOOTER_HEIGHT * scaleY,
      w: width,
      h: FOOTER_HEIGHT * scaleY,
    },
    logo: scaleRect(REFERENCE_LOGO),
  }
}
