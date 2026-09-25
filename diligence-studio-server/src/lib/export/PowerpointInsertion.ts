import JSZip from 'jszip'
import { isDefined } from '../shared/PowerpointUtils'
import {
  escapeXml,
  unescapeXmlAttribute,
} from './PowerpointUtils'

export async function insertPptxBytesIntoExistingDeck(
  slideDeckBytes: Uint8Array,
  targetDeckBytes: Uint8Array,
  targetFileName: string,
  insertAfterSlide: number,
) {
  const targetZip = await JSZip.loadAsync(targetDeckBytes)
  const sourceZip = await JSZip.loadAsync(slideDeckBytes)
  const presentationPath = 'ppt/presentation.xml'
  const presentationRelsPath = 'ppt/_rels/presentation.xml.rels'
  const contentTypesPath = '[Content_Types].xml'
  const presentationXml = await targetZip.file(presentationPath)?.async('text')
  const presentationRelsXml = await targetZip.file(presentationRelsPath)?.async('text')
  const contentTypesXml = await targetZip.file(contentTypesPath)?.async('text')

  if (!presentationXml || !presentationRelsXml || !contentTypesXml) {
    throw new Error('The selected PowerPoint file is missing required presentation metadata.')
  }

  const sourceSlideNumbers = listSlideNumbers(sourceZip)
  if (!sourceSlideNumbers.length) {
    throw new Error('No generated slide was available to add to the selected PowerPoint file.')
  }

  const targetSlideIds = parseSlideIdEntries(presentationXml)
  const boundedInsertAfterSlide = clampInteger(
    insertAfterSlide,
    0,
    targetSlideIds.length,
  )
  const targetLayout = await findDefaultSlideLayoutTarget(targetZip)
  let nextSlideNumber = Math.max(0, ...listSlideNumbers(targetZip)) + 1
  let nextSlideId = Math.max(255, ...targetSlideIds.map((slideId) => slideId.id)) + 1
  let presentationRels = parseRelationships(presentationRelsXml)
  let contentTypes = contentTypesXml
  const insertedSlideIdEntries: string[] = []
  const insertedSlideIds: number[] = []

  for (const sourceSlideNumber of sourceSlideNumbers) {
    const sourceSlidePath = `ppt/slides/slide${sourceSlideNumber}.xml`
    const sourceSlideRelsPath = `ppt/slides/_rels/slide${sourceSlideNumber}.xml.rels`
    const sourceSlideXml = await sourceZip.file(sourceSlidePath)?.async('text')

    if (!sourceSlideXml) {
      continue
    }

    const targetSlideNumber = nextSlideNumber++
    const targetSlidePath = `ppt/slides/slide${targetSlideNumber}.xml`
    const targetSlideRelsPath = `ppt/slides/_rels/slide${targetSlideNumber}.xml.rels`
    const newPresentationRelId = nextRelationshipId(presentationRels)
    const newSlideId = nextSlideId++

    targetZip.file(targetSlidePath, sourceSlideXml)
    const sourceSlideRelsXml = await sourceZip.file(sourceSlideRelsPath)?.async('text')
    const targetSlideRelsXml = sourceSlideRelsXml
      ? await rewriteSlideRelationships(sourceZip, targetZip, sourceSlideRelsXml, targetLayout)
      : buildRelationshipsXml([])
    targetZip.file(targetSlideRelsPath, targetSlideRelsXml)

    presentationRels = [
      ...presentationRels,
      {
        Id: newPresentationRelId,
        Type: 'http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide',
        Target: `slides/slide${targetSlideNumber}.xml`,
      },
    ]
    contentTypes = ensureContentTypeOverride(
      contentTypes,
      `/${targetSlidePath}`,
      'application/vnd.openxmlformats-officedocument.presentationml.slide+xml',
    )
    contentTypes = ensureMediaContentTypes(contentTypes)
    insertedSlideIdEntries.push(`<p:sldId id="${newSlideId}" r:id="${newPresentationRelId}"/>`)
    insertedSlideIds.push(newSlideId)
  }

  if (!insertedSlideIdEntries.length) {
    throw new Error('No generated slide could be copied into the selected PowerPoint file.')
  }

  const insertedPresentationXml = insertSlideIdEntries(
    presentationXml,
    insertedSlideIdEntries,
    boundedInsertAfterSlide,
  )
  targetZip.file(
    presentationPath,
    insertSectionSlideIds(
      insertedPresentationXml,
      insertedSlideIds,
      targetSlideIds,
      boundedInsertAfterSlide,
    ),
  )
  targetZip.file(presentationRelsPath, buildRelationshipsXml(presentationRels))
  targetZip.file(contentTypesPath, contentTypes)

  const bytes = await targetZip.generateAsync({
    type: 'uint8array',
    compression: 'DEFLATE',
  })

  return {
    bytes,
    fileName: buildInsertedPowerPointFileName(targetFileName),
  }
}

interface PptxRelationship {
  Id: string
  Type: string
  Target: string
  TargetMode?: string
}

function listSlideNumbers(zip: JSZip) {
  return Object.keys(zip.files)
    .map((path) => path.match(/^ppt\/slides\/slide(\d+)\.xml$/u)?.[1])
    .filter(isDefined)
    .map((value) => Number(value))
    .filter((value) => Number.isInteger(value) && value > 0)
    .sort((left, right) => left - right)
}

function parseSlideIdEntries(presentationXml: string) {
  return Array.from(presentationXml.matchAll(/<p:sldId\b[^>]*\bid="(\d+)"[^>]*\br:id="([^"]+)"[^/]*\/>/gu))
    .map((match) => ({
      id: Number(match[1]),
      relationshipId: match[2],
    }))
    .filter((entry) => Number.isInteger(entry.id) && !!entry.relationshipId)
}

function insertSlideIdEntries(
  presentationXml: string,
  slideIdEntries: string[],
  insertAfterSlide: number,
) {
  const listMatch = presentationXml.match(/<p:sldIdLst\b[^>]*>[\s\S]*?<\/p:sldIdLst>/u)
  if (!listMatch || listMatch.index === undefined) {
    throw new Error('The selected PowerPoint file is missing a slide list.')
  }

  const listXml = listMatch[0]
  const openingMatch = listXml.match(/^<p:sldIdLst\b[^>]*>/u)
  if (!openingMatch) {
    throw new Error('The selected PowerPoint slide list could not be parsed.')
  }

  const slideMatches = Array.from(listXml.matchAll(/<p:sldId\b[^>]*\/>/gu))
  const boundedInsertAfterSlide = clampInteger(insertAfterSlide, 0, slideMatches.length)
  const insertionPoint =
    boundedInsertAfterSlide === 0
      ? openingMatch[0].length
      : (slideMatches[boundedInsertAfterSlide - 1]?.index ?? 0) +
        (slideMatches[boundedInsertAfterSlide - 1]?.[0].length ?? 0)
  const updatedListXml = `${listXml.slice(0, insertionPoint)}${slideIdEntries.join('')}${listXml.slice(insertionPoint)}`

  return `${presentationXml.slice(0, listMatch.index)}${updatedListXml}${presentationXml.slice(
    listMatch.index + listXml.length,
  )}`
}

function insertSectionSlideIds(
  presentationXml: string,
  insertedSlideIds: number[],
  existingSlideIds: Array<{ id: number; relationshipId: string }>,
  insertAfterSlide: number,
) {
  if (!insertedSlideIds.length || !presentationXml.includes(':sectionLst')) {
    return presentationXml
  }

  const anchorSlideId = insertAfterSlide > 0 ? existingSlideIds[insertAfterSlide - 1]?.id : undefined
  const sectionSlideIds = insertedSlideIds.map((id) => `<p14:sldId id="${id}"/>`).join('')

  if (anchorSlideId !== undefined) {
    const anchorPattern = new RegExp(`(<p14:sldId\\b[^>]*\\bid="${anchorSlideId}"[^>]*/>)`, 'u')
    if (anchorPattern.test(presentationXml)) {
      return presentationXml.replace(anchorPattern, `$1${sectionSlideIds}`)
    }
  }

  const firstSectionListOpening = presentationXml.match(/<p14:sldIdLst\b[^>]*>/u)
  if (firstSectionListOpening?.index !== undefined) {
    const insertionPoint = firstSectionListOpening.index + firstSectionListOpening[0].length
    return `${presentationXml.slice(0, insertionPoint)}${sectionSlideIds}${presentationXml.slice(insertionPoint)}`
  }

  return presentationXml
}

function parseRelationships(xml: string): PptxRelationship[] {
  return Array.from(xml.matchAll(/<Relationship\b([^>]*)\/>/gu))
    .map((match) => {
      const attributes = parseXmlAttributes(match[1])
      if (!attributes.Id || !attributes.Type || !attributes.Target) {
        return undefined
      }

      return {
        Id: attributes.Id,
        Type: attributes.Type,
        Target: attributes.Target,
        TargetMode: attributes.TargetMode,
      }
    })
    .filter(isDefined)
}

function buildRelationshipsXml(relationships: PptxRelationship[]) {
  const relationshipXml = relationships
    .map((relationship) => {
      const targetMode = relationship.TargetMode
        ? ` TargetMode="${escapeXml(relationship.TargetMode)}"`
        : ''
      return `<Relationship Id="${escapeXml(relationship.Id)}" Type="${escapeXml(
        relationship.Type,
      )}" Target="${escapeXml(relationship.Target)}"${targetMode}/>`
    })
    .join('')

  return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">${relationshipXml}</Relationships>`
}

function parseXmlAttributes(attributesXml: string) {
  const attributes: Record<string, string> = {}
  for (const match of attributesXml.matchAll(/([\w:.-]+)="([^"]*)"/gu)) {
    attributes[match[1]] = unescapeXmlAttribute(match[2])
  }
  return attributes
}

function nextRelationshipId(relationships: PptxRelationship[]) {
  const used = new Set(relationships.map((relationship) => relationship.Id))
  let next = Math.max(
    0,
    ...relationships
      .map((relationship) => relationship.Id.match(/^rId(\d+)$/u)?.[1])
      .filter(isDefined)
      .map((value) => Number(value)),
  ) + 1

  while (used.has(`rId${next}`)) {
    next += 1
  }

  return `rId${next}`
}

async function findDefaultSlideLayoutTarget(zip: JSZip) {
  const firstSlideNumber = listSlideNumbers(zip)[0]
  if (!firstSlideNumber) {
    return undefined
  }

  const relsXml = await zip.file(`ppt/slides/_rels/slide${firstSlideNumber}.xml.rels`)?.async('text')
  const layoutRelationship = relsXml
    ? parseRelationships(relsXml).find((relationship) => relationship.Type.endsWith('/slideLayout'))
    : undefined

  return layoutRelationship?.Target
}

async function rewriteSlideRelationships(
  sourceZip: JSZip,
  targetZip: JSZip,
  sourceSlideRelsXml: string,
  targetLayout: string | undefined,
) {
  const rewrittenRelationships: PptxRelationship[] = []

  for (const relationship of parseRelationships(sourceSlideRelsXml)) {
    if (relationship.Type.endsWith('/slideLayout') && targetLayout) {
      rewrittenRelationships.push({
        ...relationship,
        Target: targetLayout,
      })
      continue
    }

    if (relationship.Type.endsWith('/notesSlide')) {
      continue
    }

    if (relationship.Type.includes('/image')) {
      const copiedMediaTarget = await copySlideRelationshipMedia(sourceZip, targetZip, relationship.Target)
      if (copiedMediaTarget) {
        rewrittenRelationships.push({
          ...relationship,
          Target: copiedMediaTarget,
        })
      }
      continue
    }

    rewrittenRelationships.push(relationship)
  }

  return buildRelationshipsXml(rewrittenRelationships)
}

async function copySlideRelationshipMedia(sourceZip: JSZip, targetZip: JSZip, relationshipTarget: string) {
  const sourcePath = resolvePptxPartPath('ppt/slides', relationshipTarget)
  const sourceFile = sourceZip.file(sourcePath)
  if (!sourceFile) {
    return undefined
  }

  const extension = sourcePath.split('.').pop()?.toLowerCase() || 'bin'
  const targetFileName = nextMediaFileName(targetZip, extension)
  targetZip.file(`ppt/media/${targetFileName}`, await sourceFile.async('uint8array'))
  return `../media/${targetFileName}`
}

function nextMediaFileName(zip: JSZip, extension: string) {
  const escapedExtension = extension.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const existingNumbers = Object.keys(zip.files)
    .map((path) => path.match(new RegExp(`^ppt/media/image(\\d+)\\.${escapedExtension}$`, 'u'))?.[1])
    .filter(isDefined)
    .map((value) => Number(value))
  const nextNumber = Math.max(0, ...existingNumbers) + 1
  return `image${nextNumber}.${extension}`
}

function resolvePptxPartPath(basePath: string, target: string) {
  const segments = [...basePath.split('/'), ...target.split('/')]
  const resolved: string[] = []

  for (const segment of segments) {
    if (!segment || segment === '.') {
      continue
    }
    if (segment === '..') {
      resolved.pop()
      continue
    }
    resolved.push(segment)
  }

  return resolved.join('/')
}

function ensureContentTypeOverride(xml: string, partName: string, contentType: string) {
  if (xml.includes(`PartName="${partName}"`)) {
    return xml
  }

  return xml.replace(
    '</Types>',
    `<Override PartName="${escapeXml(partName)}" ContentType="${escapeXml(contentType)}"/></Types>`,
  )
}

function ensureMediaContentTypes(xml: string) {
  return [
    ['png', 'image/png'],
    ['jpg', 'image/jpeg'],
    ['jpeg', 'image/jpeg'],
  ].reduce((updatedXml, [extension, contentType]) => {
    if (new RegExp(`<Default\\b[^>]*Extension="${extension}"`, 'u').test(updatedXml)) {
      return updatedXml
    }

    return updatedXml.replace(
      '</Types>',
      `<Default Extension="${extension}" ContentType="${contentType}"/></Types>`,
    )
  }, xml)
}

function buildInsertedPowerPointFileName(fileName: string) {
  const trimmed = fileName
    .replace(/[^A-Za-z0-9._ ()-]/gu, '-')
    .replace(/-+/gu, '-')
    .trim()
  const stem = trimmed.toLowerCase().endsWith('.pptx') ? trimmed.slice(0, -5) : trimmed
  return `${stem || 'presentation'}-with-slide.pptx`
}

function clampInteger(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) {
    return max
  }

  return Math.max(min, Math.min(max, Math.trunc(value)))
}
