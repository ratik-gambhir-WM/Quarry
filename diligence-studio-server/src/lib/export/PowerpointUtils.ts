import type { VerticalAlign } from '../shared/PowerpointTypes'
import { PX_PER_INCH } from '../shared/PowerpointConstants'
import { clamp01, normalizeShapeName } from '../shared/PowerpointUtils'
import { PPTX_FULL_TRANSPARENCY } from './PowerpointConstants'

export function toPptxShapeName(shape: string) {
  return normalizeShapeName(shape) as never
}

export function toPptxVerticalAlign(value: VerticalAlign) {
  if (value === 'top') {
    return 'top'
  }

  if (value === 'bottom') {
    return 'bottom'
  }

  return 'middle'
}

export function opacityToTransparency(opacity: number) {
  return Math.round((1 - clamp01(opacity)) * PPTX_FULL_TRANSPARENCY)
}

export function pxToInches(px: number) {
  return px / PX_PER_INCH
}

export function slugify(value: string) {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

export function escapeXml(input: string) {
  return input
    .replace(/&/g, '&amp;')
    .replace(/"/g, '&quot;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

export function unescapeXmlAttribute(input: string) {
  return input
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
}
