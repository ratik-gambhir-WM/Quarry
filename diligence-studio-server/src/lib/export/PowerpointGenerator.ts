import JSZip from 'jszip'

import type { NormalizedPresentation } from '../shared/PowerpointTypes'
import { normalizePresentationSpec } from '../shared/PowerpointNormalizer'
import { buildPptxPresentation } from './PowerpointRenderer'
import { applyDefaultThemeXml } from './PowerpointTheme'
import { slugify } from './PowerpointUtils'

export { normalizePresentationSpec }

export function buildSuggestedFileName(presentation: NormalizedPresentation) {
  const stem = slugify(presentation.meta.title || 'generated-presentation')
  return `${stem || 'generated-presentation'}.pptx`
}

export async function buildThemedPptxBytes(presentation: NormalizedPresentation) {
  const raw = await buildPptxPresentation(presentation).write({
    outputType: 'uint8array',
    compression: true,
  })
  const zip = await JSZip.loadAsync(raw)
  const themePath = 'ppt/theme/theme1.xml'
  const existingTheme = await zip.file(themePath)?.async('text')
  if (existingTheme) {
    zip.file(themePath, applyDefaultThemeXml(existingTheme))
  }
  return zip.generateAsync({ type: 'uint8array', compression: 'DEFLATE' })
}
