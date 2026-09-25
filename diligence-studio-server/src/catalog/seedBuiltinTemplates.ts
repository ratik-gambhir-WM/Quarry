import { createHash } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

import type { PowerPointCanvasJson } from '../lib/import/PowerpointImportTypes'
import type { TemplateKind, TemplateRepository } from '../repositories/TemplateRepository'
import { externalizeTemplateAssets } from '../services/TemplateAssets'
import { readPngDimensions } from '../services/TemplatePreview'
import { normalizePresentation } from '../services/NormalizePresentation'

type CatalogEntry = {
  description: string
  json: string
  kind: TemplateKind
  preview: string
  templateId: string
  title: string
}

type CatalogManifest = {
  templates: CatalogEntry[]
  version: number
}

const CATALOG_ROOT = new URL('./', import.meta.url)
const BUILTIN_CREATED_AT = '2026-01-01T00:00:00.000Z'

export async function seedBuiltinTemplates(repository: TemplateRepository) {
  const manifestBytes = await readFile(new URL('manifest.json', CATALOG_ROOT))
  const manifest = parseManifest(JSON.parse(manifestBytes.toString('utf8')) as unknown)

  for (const entry of manifest.templates) {
    const jsonBytes = await readCatalogFile(entry.json)
    const previewBytes = await readCatalogFile(entry.preview)
    const dimensions = readPngDimensions(previewBytes)
    if (!dimensions) {
      throw new Error(`Built-in template preview is not a valid PNG: ${entry.templateId}`)
    }

    const parsedTemplate = parseCanvasDocument(JSON.parse(jsonBytes.toString('utf8')) as unknown)
    parsedTemplate.presentation.title = entry.title
    const normalizedTemplate = entry.kind === 'commentary'
      ? removeLegacyCommentaryFrame(parsedTemplate)
      : parsedTemplate
    let assetIndex = 0
    const externalized = externalizeTemplateAssets(
      entry.templateId,
      normalizedTemplate,
      () => `${entry.templateId}-asset-${assetIndex += 1}`,
    )
    const checksum = createHash('sha256')
      .update(String(manifest.version))
      .update(jsonBytes)
      .update(previewBytes)
      .update(JSON.stringify(entry))
      .digest('hex')

    repository.upsertBuiltin(
      { templateId: entry.templateId, templateJson: externalized.templateJson },
      externalized.assets,
      {
        checksum,
        createdAt: BUILTIN_CREATED_AT,
        description: entry.description,
        kind: entry.kind,
        source: 'builtin',
        templateId: entry.templateId,
      },
      {
        bytes: previewBytes,
        contentType: 'image/png',
        ...dimensions,
        templateId: entry.templateId,
      },
    )
  }
}

async function readCatalogFile(relativePath: string) {
  const fileUrl = new URL(relativePath, CATALOG_ROOT)
  if (!fileURLToPath(fileUrl).startsWith(fileURLToPath(CATALOG_ROOT))) {
    throw new Error('Built-in catalog paths must stay inside the catalog directory.')
  }
  return readFile(fileUrl)
}

function parseManifest(value: unknown): CatalogManifest {
  if (!isRecord(value) || !Number.isInteger(value.version) || !Array.isArray(value.templates)) {
    throw new Error('The built-in template catalog manifest is invalid.')
  }
  const templates = value.templates.map((entry) => {
    if (
      !isRecord(entry)
      || !isTemplateKind(entry.kind)
      || !isNonEmptyString(entry.templateId)
      || !isNonEmptyString(entry.title)
      || !isNonEmptyString(entry.description)
      || !isNonEmptyString(entry.json)
      || !isNonEmptyString(entry.preview)
    ) {
      throw new Error('The built-in template catalog contains an invalid entry.')
    }
    return {
      description: entry.description,
      json: entry.json,
      kind: entry.kind,
      preview: entry.preview,
      templateId: entry.templateId,
      title: entry.title,
    }
  })
  return { templates, version: value.version as number }
}

function parseCanvasDocument(value: unknown): PowerPointCanvasJson {
  try {
    return normalizePresentation(value).templateJson
  } catch {
    throw new Error('A built-in template JSON file is invalid.')
  }
}

function removeLegacyCommentaryFrame(template: PowerPointCanvasJson): PowerPointCanvasJson {
  const frameIds = new Set(['element-903000', 'west-monroe-footer', 'west-monroe-logo'])
  return {
    ...template,
    presentation: {
      ...template.presentation,
      slides: template.presentation.slides.map((slide) => ({
        ...slide,
        elements: slide.elements.filter((element) => {
          if (frameIds.has(element.id) || element.id.startsWith('west-monroe-dot-')) {
            return false
          }
          return element.type !== 'image'
            || (!element.src.includes('element-903000.jpg') && !element.src.includes('element-5.png'))
        }),
      })),
    },
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isTemplateKind(value: unknown): value is TemplateKind {
  return value === 'diagram' || value === 'commentary'
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}
