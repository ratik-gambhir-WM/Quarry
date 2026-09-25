import path from 'node:path'
import type JSZip from 'jszip'

import {
  DEFAULT_SLIDE_HEIGHT_EMU,
  DEFAULT_SLIDE_WIDTH_EMU,
  EMU_PER_INCH,
  PX_PER_INCH,
} from '../shared/PowerpointConstants'
import type { ExtractedRelationship } from '../shared/PowerpointTypes'
import type { ExtractedSupportPartRecord } from './PowerpointImportTypes'
import { round } from './PowerpointImportUtils'
import { descendants, findDescendant, parseXml } from './PowerpointXml'

export async function collectSupportParts(zip: JSZip, relationships: ExtractedRelationship[]) {
  const supportParts: Record<string, ExtractedSupportPartRecord> = {}

  for (const relationship of relationships) {
    if (!relationship.resolvedTarget) {
      continue
    }

    if (relationship.Type?.includes('/theme')) {
      const rawXml = await maybeReadZipText(zip, relationship.resolvedTarget)
      if (rawXml) {
        supportParts[relationship.resolvedTarget] = {
          path: relationship.resolvedTarget,
          size: Buffer.byteLength(rawXml),
          relationshipType: relationship.Type,
          contentTypeHint: 'application/xml',
          rawXml,
        }
      }
      continue
    }

    if (!relationship.Type?.includes('/image')) {
      continue
    }

    const bytes = await zip.file(relationship.resolvedTarget)?.async('nodebuffer')
    if (!bytes) {
      continue
    }

    supportParts[relationship.resolvedTarget] = {
      path: relationship.resolvedTarget,
      size: bytes.length,
      relationshipType: relationship.Type,
      contentTypeHint: imageContentType(relationship.resolvedTarget),
      base64: bytes.toString('base64'),
    }
  }

  if (!Object.values(supportParts).some((part) => part.relationshipType.includes('/theme'))) {
    const fallbackThemePath = Object.keys(zip.files)
      .sort()
      .find((partPath) => partPath.startsWith('ppt/theme/') && partPath.endsWith('.xml'))
    const rawXml = fallbackThemePath ? await maybeReadZipText(zip, fallbackThemePath) : undefined
    if (fallbackThemePath && rawXml) {
      supportParts[fallbackThemePath] = {
        path: fallbackThemePath,
        size: Buffer.byteLength(rawXml),
        relationshipType:
          'http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme',
        contentTypeHint: 'application/xml',
        rawXml,
      }
    }
  }

  return supportParts
}

export function extractSlidePaths(presentationXml: string, relationships: ExtractedRelationship[]) {
  const presentation = parseXml(presentationXml)
  const relById = new Map(relationships.map((relationship) => [relationship.Id, relationship]))
  return descendants(presentation, 'p:sldId')
    .map((slideId) => relById.get(slideId.attributes?.['r:id'] ?? '')?.resolvedTarget)
    .filter((target): target is string => !!target)
}

export function extractSlideSize(presentationXml: string) {
  const presentation = parseXml(presentationXml)
  const sldSz = findDescendant(presentation, 'p:sldSz')
  const cx = Number(sldSz?.attributes?.cx) || DEFAULT_SLIDE_WIDTH_EMU
  const cy = Number(sldSz?.attributes?.cy) || DEFAULT_SLIDE_HEIGHT_EMU
  return {
    cx,
    cy,
    widthPx: round((cx / EMU_PER_INCH) * PX_PER_INCH),
    heightPx: round((cy / EMU_PER_INCH) * PX_PER_INCH),
  }
}

export function parseRelationships(xml: string, baseDir: string): ExtractedRelationship[] {
  const root = parseXml(xml)
  return descendants(root, 'Relationship').map((relationship) => {
    const Type = relationship.attributes?.Type
    const Target = relationship.attributes?.Target
    return {
      Id: relationship.attributes?.Id ?? '',
      Type,
      Target,
      resolvedTarget: Target ? resolvePartPath(baseDir, Target) : undefined,
      typeShort: Type?.split('/').pop(),
    }
  })
}

export async function readZipText(zip: JSZip, filePath: string) {
  const file = zip.file(filePath)
  if (!file) {
    throw new Error(`Missing expected PowerPoint part: ${filePath}`)
  }
  return file.async('text')
}

export async function maybeReadZipText(zip: JSZip, filePath: string) {
  return zip.file(filePath)?.async('text')
}

export function relationshipPathFor(partPath: string) {
  const directory = path.posix.dirname(partPath)
  const base = path.posix.basename(partPath)
  return path.posix.join(directory, '_rels', `${base}.rels`)
}

function resolvePartPath(baseDir: string, target: string) {
  if (/^[a-z]+:/iu.test(target)) {
    return target
  }
  return path.posix.normalize(path.posix.join(baseDir, target))
}

function imageContentType(filePath: string) {
  const extension = path.extname(filePath).toLowerCase()
  if (extension === '.jpg' || extension === '.jpeg') {
    return 'image/jpeg'
  }
  if (extension === '.svg') {
    return 'image/svg+xml'
  }
  if (extension === '.gif') {
    return 'image/gif'
  }
  if (extension === '.emf') {
    return 'image/emf'
  }
  return 'image/png'
}
